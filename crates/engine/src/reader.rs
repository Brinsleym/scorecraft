//! The reader: which pages of the open score are shown, in what order and layout, where the
//! player is, and the frame a frontend draws (feature-map READ-2..6, READ-12, READ-15, TURN-8;
//! spec `docs/spec/page-turning.md`).
//!
//! The reader turns through *score pages*: for a library score, its page list in order with
//! hidden pages and pages outside its page range left out (PAGE-1..4, PAGE-8); for a loose file,
//! the file's pages. Each score page names a *file page* (what the renderer draws) and an extra
//! rotation the frontend applies when it draws the bitmap.

use std::sync::Arc;

use scorecraft_doc::Score;
use scorecraft_geom::{Rect, Rotation, Size};
use scorecraft_layout::{Frame, Reader, Slot, View, Zoom, place};
use scorecraft_pdf::{Bitmap, Document};
use serde_json::{Value, json};

use crate::{EngineError, Session};

/// One score page as the reader shows it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShownPage {
    /// 0-based page of the file.
    pub file_page: usize,
    /// Rotation added on top of the file's own `/Rotate`; the frontend turns the bitmap by it.
    pub rotation: Rotation,
    /// Displayed size in points, after both rotations.
    pub size: Size,
}

/// The reader area before a frontend reports its own: a 1280 × 800 window at 1×.
const DEFAULT_VIEWPORT: Size = Size {
    width: 1280.0,
    height: 800.0,
};

/// Largest viewport side accepted, in pixels.
pub const MAX_VIEWPORT: f64 = 16_384.0;
/// Highest device pixels per frontend pixel accepted.
pub const MAX_PIXELS_PER_POINT: f64 = 8.0;

/// The open score's reader state.
#[derive(Debug, Clone)]
pub struct ReaderState {
    pub(crate) reader: Reader,
    pub(crate) shown: Vec<ShownPage>,
    pub(crate) zoom: Zoom,
    /// The reader area in frontend pixels.
    pub(crate) viewport: Size,
    /// Device pixels per frontend pixel (2 on a 2× display).
    pub(crate) pixels_per_point: f64,
}

impl ReaderState {
    pub(crate) fn new(shown: Vec<ShownPage>) -> ReaderState {
        ReaderState {
            reader: Reader::new(shown.len()),
            shown,
            zoom: Zoom::default(),
            viewport: DEFAULT_VIEWPORT,
            pixels_per_point: 1.0,
        }
    }

    /// Replaces the page list (after a page edit), staying on the same file page when it is still
    /// shown, else on the nearest score page.
    pub(crate) fn reshow(&mut self, shown: Vec<ShownPage>) {
        if shown == self.shown {
            return;
        }
        let here = self.reader.page();
        let file_page = self.shown.get(here).map(|p| p.file_page);
        let target = file_page
            .and_then(|f| shown.iter().position(|p| p.file_page == f))
            .unwrap_or(here);
        let (layout, mode, split) = (
            self.reader.layout(),
            self.reader.mode(),
            self.reader.split(),
        );
        let mut reader = Reader::new(shown.len());
        reader.set_layout(layout);
        reader.set_mode(mode);
        reader.set_split(split);
        reader.goto(target);
        self.reader = reader;
        self.shown = shown;
    }

    /// The file page being read; 0 when nothing is shown.
    pub(crate) fn current_file_page(&self) -> usize {
        self.shown
            .get(self.reader.page())
            .map_or(0, |p| p.file_page)
    }

    /// Goes to the first score page showing file page `file_page`; false if none does (it is
    /// hidden or outside the page range).
    pub(crate) fn goto_file_page(&mut self, file_page: usize) -> bool {
        match self.shown.iter().position(|p| p.file_page == file_page) {
            Some(i) => {
                self.reader.goto(i);
                self.zoom.pan.1 = 0.0;
                true
            }
            None => false,
        }
    }

    fn sizes(&self) -> Vec<Size> {
        self.shown.iter().map(|p| p.size).collect()
    }

    fn place_view(&self, view: &View) -> Frame {
        place(
            view,
            &self.sizes(),
            self.viewport,
            self.reader.split(),
            self.zoom,
        )
    }

    /// The current view placed in the viewport.
    pub(crate) fn frame(&self) -> Frame {
        match self.reader.view() {
            Some(v) => self.place_view(&v),
            None => Frame {
                slots: Vec::new(),
                overflow: (0.0, 0.0),
            },
        }
    }

    /// The device pixels per point a slot's file page renders at.
    pub(crate) fn render_scale(&self, slot: &Slot) -> f32 {
        (slot.scale * self.pixels_per_point) as f32
    }

    /// File pages and render scales to pre-render: the current view, the next, the previous.
    pub(crate) fn prefetch_list(&self) -> Vec<(usize, f32)> {
        let (prev, next) = self.reader.neighbours();
        let mut out: Vec<(usize, f32)> = Vec::new();
        let views = [self.reader.view(), next, prev];
        for v in views.iter().flatten() {
            // Neighbours are drawn unzoomed: a turn resets the pan, never the zoom, so use the
            // current zoom with the pan at the top.
            for slot in self.place_view(v).slots {
                if let Some(p) = self.shown.get(slot.page) {
                    let item = (p.file_page, self.render_scale(&slot));
                    if !out.iter().any(|o| o.0 == item.0 && o.1 == item.1) {
                        out.push(item);
                    }
                }
            }
        }
        out
    }

    /// The reader state and frame as agents read it. Page numbers are 1-based.
    pub(crate) fn to_json(&self) -> Value {
        let frame = self.frame();
        let view = self.reader.view();
        let slots: Vec<Value> = frame
            .slots
            .iter()
            .map(|s| {
                let shown = self.shown.get(s.page);
                json!({
                    "page": s.page + 1,
                    "filePage": shown.map(|p| p.file_page + 1),
                    "rotation": shown.map_or(0, |p| p.rotation.degrees()),
                    "src": rect_json(s.src),
                    "dst": rect_json(s.dst),
                    "scale": s.scale,
                })
            })
            .collect();
        let first_alone = matches!(
            self.reader.layout(),
            scorecraft_layout::Layout::TwoPage { first_alone: true }
        );
        json!({
            "pageCount": self.shown.len(),
            "page": if self.shown.is_empty() { Value::Null } else { json!(self.reader.page() + 1) },
            "onScreen": view.as_ref().map(|v| {
                let mut pages: Vec<usize> = v.pages().iter().map(|p| p + 1).collect();
                pages.sort_unstable();
                pages
            }),
            "half": view.as_ref().is_some_and(View::is_half),
            "turn": self.reader.index() + 1,
            "turns": self.reader.len(),
            "layout": self.reader.layout().id(),
            "firstAlone": first_alone,
            "mode": self.reader.mode().id(),
            "split": self.reader.split(),
            "zoom": {
                "fit": self.zoom.fit.id(),
                "factor": self.zoom.factor,
                "pan": [self.zoom.pan.0, self.zoom.pan.1],
            },
            "viewport": {
                "width": self.viewport.width,
                "height": self.viewport.height,
                "pixelsPerPoint": self.pixels_per_point,
            },
            "overflow": [frame.overflow.0, frame.overflow.1],
            "slots": slots,
        })
    }
}

fn rect_json(r: Rect) -> Value {
    json!([r.x0, r.y0, r.x1, r.y1])
}

/// The size of a file page turned by `extra`.
fn turned(doc: &Document, file_page: usize, extra: Rotation) -> Option<Size> {
    let g = doc.page(file_page).ok()?;
    let (w, h) = (f64::from(g.width), f64::from(g.height));
    Some(if extra.quarters() % 2 == 1 {
        Size {
            width: h,
            height: w,
        }
    } else {
        Size {
            width: w,
            height: h,
        }
    })
}

/// Every page of a loose file, as the file has them.
pub(crate) fn file_pages(doc: &Document) -> Vec<ShownPage> {
    (0..doc.page_count())
        .filter_map(|i| {
            Some(ShownPage {
                file_page: i,
                rotation: Rotation::R0,
                size: turned(doc, i, Rotation::R0)?,
            })
        })
        .collect()
}

/// A library score's shown pages: its page list within the page range, without hidden pages
/// and without pages the file doesn't have. With `range` (a setlist entry's), only page
/// positions inside it are kept; a range that keeps nothing keeps everything, so the reader is
/// never left without a page.
pub(crate) fn score_pages_within(
    doc: &Document,
    score: &Score,
    range: Option<scorecraft_doc::PageRange>,
) -> Vec<ShownPage> {
    let mut visible = score.visible_pages();
    if let Some(r) = range {
        let kept: Vec<usize> = visible
            .iter()
            .copied()
            .filter(|i| u32::try_from(*i).is_ok_and(|i| r.contains(i)))
            .collect();
        if !kept.is_empty() {
            visible = kept;
        }
    }
    visible
        .into_iter()
        .filter_map(|i| {
            let p = score.pages.get(i)?;
            let file_page = usize::try_from(p.source.0).ok()?;
            Some(ShownPage {
                file_page,
                rotation: p.rotation,
                size: turned(doc, file_page, p.rotation)?,
            })
        })
        .collect()
}

/// One placed page part with its bitmap, for a frontend to draw.
#[derive(Debug, Clone)]
pub struct FrameSlot {
    /// 0-based score page.
    pub page: usize,
    /// 0-based file page.
    pub file_page: usize,
    /// Turn the bitmap by this before drawing; `src` is in the turned page.
    pub rotation: Rotation,
    /// The part of the (turned) page shown, as fractions of its width and height.
    pub src: Rect,
    /// Where it goes, in the frontend pixels of the viewport set with `view.viewport.set`.
    pub dst: Rect,
    /// The whole file page; `None` if it isn't cached and rendering wasn't asked for, or the
    /// render failed (the frontend draws a placeholder).
    pub bitmap: Option<Arc<Bitmap>>,
}

impl Session {
    /// Rebuilds the open library score's page list when page edits changed it.
    pub(crate) fn refresh_reader(&mut self) {
        let Some(open) = self.score.as_ref() else {
            return;
        };
        let Some(id) = open.library_score else {
            return;
        };
        // While a setlist plays, its entry's page range narrows the score's pages (LIST-2).
        let range = self
            .playback
            .filter(|p| p.score == id)
            .and_then(|p| p.range);
        let shown = self
            .library()
            .and_then(|lib| lib.scores.get(&id))
            .map(|s| score_pages_within(open.view.document(), s, range));
        if let (Some(shown), Some(open)) = (shown, self.score.as_mut()) {
            open.reader.reshow(shown);
        }
    }

    /// The reader state of the open score.
    pub(crate) fn reader_mut(&mut self, command: &str) -> Result<&mut ReaderState, EngineError> {
        self.refresh_reader();
        self.score
            .as_mut()
            .map(|s| &mut s.reader)
            .ok_or_else(|| EngineError::Disabled(command.to_owned()))
    }

    /// Queues the pages a turn will show next for background rendering.
    pub(crate) fn prefetch_reader(&mut self) {
        let lib = self.library.as_ref().map(|l| &l.lib);
        let curve = self.markup.curve;
        if let Some(open) = self.score.as_mut() {
            let list = open.reader.prefetch_list();
            let score = crate::markup::library_score(lib, open.library_score);
            crate::markup::sync_ink(&open.view, score, curve, list.iter().map(|p| p.0));
            open.view.prefetch_each(&list);
        }
    }

    /// The frame to draw now: every visible page part with its bitmap. With `render`, pages not
    /// yet cached are rendered first (the first frame after opening); without it a turn never
    /// waits on rendering and missing bitmaps come back as `None`. `None` with no score open.
    pub fn reader_frame(&mut self, render: bool) -> Option<Vec<FrameSlot>> {
        self.refresh_reader();
        let lib = self.library.as_ref().map(|l| &l.lib);
        let curve = self.markup.curve;
        let open = self.score.as_mut()?;
        let score = crate::markup::library_score(lib, open.library_score);
        let on_screen: Vec<usize> = open
            .reader
            .frame()
            .slots
            .iter()
            .filter_map(|s| open.reader.shown.get(s.page).map(|p| p.file_page))
            .collect();
        crate::markup::sync_ink(&open.view, score, curve, on_screen);
        let frame = open.reader.frame();
        let mut out = Vec::with_capacity(frame.slots.len());
        for slot in frame.slots {
            let Some(shown) = open.reader.shown.get(slot.page).copied() else {
                continue;
            };
            let scale = open.reader.render_scale(&slot);
            // A page cached without its ink yet (just marked) is drawn now: a page is never
            // shown without its ink.
            let bitmap = if render || open.view.is_cached(shown.file_page, scale) {
                open.view.inked_page(shown.file_page, scale).ok()
            } else {
                None
            };
            out.push(FrameSlot {
                page: slot.page,
                file_page: shown.file_page,
                rotation: shown.rotation,
                src: slot.src,
                dst: slot.dst,
                bitmap,
            });
        }
        Some(out)
    }

    /// Waits until background pre-rendering has caught up, up to `timeout` (native only). For
    /// tests, benchmarks and screenshots; a frontend never waits.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait_for_prefetch(&self, timeout: std::time::Duration) -> bool {
        self.score
            .as_ref()
            .is_none_or(|s| s.view.wait_for_prefetch(timeout))
    }

    /// The reader state as `view.state` returns it; `None` with no score open.
    pub fn reader_state(&mut self) -> Option<Value> {
        self.refresh_reader();
        self.score.as_ref().map(|s| s.reader.to_json())
    }
}
