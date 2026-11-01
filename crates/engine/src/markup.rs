//! Markup state and ink on the reader's pages (spec `docs/spec/ink.md`).
//!
//! [`Markup`] is what the toolbar shows: the active tool, each tool's colour and width, the
//! eraser mode, *Draw with Touch* and the active layer per score. [`sync_ink`] tells the page
//! cache what ink each page carries, keyed by a fingerprint of its visible markings; the cache
//! draws it once per change, in the background for the pages a turn will show, never per frame
//! (AGENTS.md rule 8).

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use scorecraft_doc::{AnnotationKind, Color, LayerId, Library, Nib, PageKey, Score, ScoreId, Tool};
use scorecraft_geom::{PageBox, Point, Rect, Rotation};
use scorecraft_ink::{
    ERASER_WIDTHS, HIGHLIGHTER_PALETTE, HIGHLIGHTER_SWATCHES, PALETTE, PressureCurve, Ruler,
    SWATCHES, ToolStyle, WHITEOUT_SWATCH,
};
use scorecraft_pdf::Document;
use scorecraft_render::ink::{is_drawn, page_box, visible_marks};
use scorecraft_render::{PageInk, ScoreView};
use serde_json::{Value, json};

use crate::reader::ReaderState;
use crate::snapshot::unturn;

/// The toolbar's tools (spec, "The markup toolbar").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkupTool {
    Pen,
    Highlighter,
    Eraser,
    Text,
    Stamp,
    /// Rectangle select (ANNO-12).
    Select,
}

impl MarkupTool {
    /// Every tool, in the toolbar's order.
    pub const ALL: [MarkupTool; 6] = [
        MarkupTool::Pen,
        MarkupTool::Highlighter,
        MarkupTool::Eraser,
        MarkupTool::Text,
        MarkupTool::Stamp,
        MarkupTool::Select,
    ];

    pub fn id(self) -> &'static str {
        match self {
            MarkupTool::Pen => "pen",
            MarkupTool::Highlighter => "highlighter",
            MarkupTool::Eraser => "eraser",
            MarkupTool::Text => "text",
            MarkupTool::Stamp => "stamp",
            MarkupTool::Select => "select",
        }
    }

    pub fn from_id(id: &str) -> Option<MarkupTool> {
        MarkupTool::ALL.into_iter().find(|t| t.id() == id)
    }
}

/// The markings a rectangle selected (ANNO-12): on one page of one score, by id. Undo can take
/// markings away under it, so every action keeps only the ids still on the page.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub score: ScoreId,
    pub page: PageKey,
    /// The rectangle, in page space; it follows the markings when they move or scale.
    pub rect: Rect,
    pub ids: Vec<scorecraft_doc::AnnotationId>,
}

/// The eraser's two modes (ANNO-6, ANNO-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraserMode {
    Stroke,
    Pixel,
}

impl EraserMode {
    pub fn id(self) -> &'static str {
        match self {
            EraserMode::Stroke => "stroke",
            EraserMode::Pixel => "pixel",
        }
    }
}

/// A saved tool, colour and width (ANNO-4).
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub name: String,
    pub tool: Tool,
    pub color: Color,
    pub width: f32,
}

/// Most presets kept.
pub const MAX_PRESETS: usize = 50;

/// Markup settings. Session state, like the pedal map: the shell saves and restores it with
/// `annotate.state` and the tool commands.
#[derive(Debug, Clone)]
pub struct Markup {
    pub tool: MarkupTool,
    pub pen: ToolStyle,
    pub highlighter: ToolStyle,
    pub whiteout: ToolStyle,
    /// The pen paints whiteout (ANNO-5) instead of ink.
    pub whiteout_on: bool,
    pub eraser: EraserMode,
    /// Eraser diameter in points.
    pub eraser_width: f32,
    /// Only a pen marks up; mouse and touch don't (ANNO-14). *Draw with Touch* is its opposite.
    pub pen_only: bool,
    /// Markup mode: the toolbar is shown and the pen draws.
    pub mode: bool,
    /// Touch lock (the padlock): in markup mode, every touch is ignored (ANNO-14).
    pub touch_lock: bool,
    /// The pen's and the highlighter's eight swatches, each from its own palette (ANNO-4).
    pub pen_swatches: [Color; 8],
    pub highlighter_swatches: [Color; 8],
    /// The selected swatch of the pen and of the highlighter.
    pub pen_swatch: usize,
    pub highlighter_swatch: usize,
    /// The *Round highlighter ends* setting: new highlighter strokes get a round nib instead of
    /// the chisel (ANNO-3).
    pub highlighter_round: bool,
    /// The ruler, if shown: the vertical centre of its band in viewport pixels (ANNO-17). It lies
    /// on the screen, not on a page.
    pub ruler: Option<f64>,
    pub curve: PressureCurve,
    /// The layer new markings go into, per score; the score's top layer when unset.
    pub layers: BTreeMap<ScoreId, LayerId>,
    /// Saved presets, in the order they were made.
    pub presets: Vec<Preset>,
    /// New text notes' size in points and colour (ANNO-8).
    pub text_size: f32,
    pub text_color: Color,
    /// The selected stamp and its size in points (ANNO-9).
    pub stamp: &'static str,
    pub stamp_size: f32,
    /// Shapes (ANNO-16): pen and highlighter strokes that are nearly a line or an ellipse are
    /// straightened.
    pub shapes: bool,
    /// The pen button's tool (ANNO-11): pressing the stylus button switches to it, and pressing
    /// again returns to the tool before.
    pub button_tool: MarkupTool,
    /// The tool the pen button returns to.
    pub button_return: Option<MarkupTool>,
    /// What the rectangle tool selected, if anything.
    pub selection: Option<Selection>,
}

/// The stamp tool's choices, in its bar's order: the dynamics, then the hairpins behind ">".
pub const STAMPS: [&str; 8] = ["pp", "p", "mp", "mf", "f", "ff", "crescendo", "diminuendo"];

/// Stamp sizes on the size slider.
pub const STAMP_SIZES: scorecraft_ink::WidthRange = scorecraft_ink::WidthRange {
    min: 8.0,
    max: 96.0,
};

/// Text sizes on the size slider (spec ANNO-8).
pub const TEXT_SIZES: scorecraft_ink::WidthRange = scorecraft_ink::WidthRange {
    min: 8.0,
    max: 36.0,
};

impl Default for Markup {
    fn default() -> Self {
        Markup {
            tool: MarkupTool::Pen,
            pen: ToolStyle::default_for(Tool::Pen),
            highlighter: ToolStyle::default_for(Tool::Highlighter),
            whiteout: ToolStyle::default_for(Tool::Whiteout),
            whiteout_on: false,
            eraser: EraserMode::Stroke,
            eraser_width: 8.0,
            pen_only: true,
            mode: false,
            touch_lock: false,
            pen_swatches: SWATCHES,
            highlighter_swatches: HIGHLIGHTER_SWATCHES,
            pen_swatch: 0,
            highlighter_swatch: 0,
            highlighter_round: false,
            ruler: None,
            curve: PressureCurve::default(),
            layers: BTreeMap::new(),
            presets: Vec::new(),
            text_size: 12.0,
            text_color: Color::BLACK,
            stamp: "p",
            stamp_size: 18.0,
            shapes: false,
            button_tool: MarkupTool::Eraser,
            button_return: None,
            selection: None,
        }
    }
}

impl Markup {
    /// The stroke tool the pen or highlighter draws with now, if the active tool draws.
    pub fn drawing(&self) -> Option<(Tool, ToolStyle)> {
        match self.tool {
            MarkupTool::Pen if self.whiteout_on => Some((Tool::Whiteout, self.whiteout)),
            MarkupTool::Pen => Some((Tool::Pen, self.pen)),
            MarkupTool::Highlighter => Some((Tool::Highlighter, self.highlighter)),
            _ => None,
        }
    }

    /// Gives the active drawing tool `color`. The pen in the palette's white paints whiteout
    /// (ANNO-5); any other colour returns it to ink. Returns false when the active tool has no
    /// colour (the eraser).
    pub fn apply_color(&mut self, color: Color) -> bool {
        match self.tool {
            MarkupTool::Pen => {
                if Color::rgb(color.r, color.g, color.b) == WHITEOUT_SWATCH {
                    self.whiteout_on = true;
                } else {
                    self.whiteout_on = false;
                    self.pen.color = color;
                }
                true
            }
            MarkupTool::Highlighter => {
                self.highlighter.color = ToolStyle::color_for(Tool::Highlighter, color);
                true
            }
            MarkupTool::Text => {
                self.text_color = Color::rgb(color.r, color.g, color.b);
                true
            }
            // Stamps are in the pen's colour.
            MarkupTool::Stamp => {
                self.pen.color = Color::rgb(color.r, color.g, color.b);
                true
            }
            MarkupTool::Eraser | MarkupTool::Select => false,
        }
    }

    /// The pen button (ANNO-11): from any tool but the button's, switches to the button's tool;
    /// from the button's tool, back to the one before (the pen the first time).
    pub fn press_button(&mut self) {
        if self.tool == self.button_tool {
            let other = if self.button_tool == MarkupTool::Pen {
                MarkupTool::Eraser
            } else {
                MarkupTool::Pen
            };
            self.tool = self
                .button_return
                .take()
                .filter(|t| *t != self.button_tool)
                .unwrap_or(other);
        } else {
            self.button_return = Some(self.tool);
            self.tool = self.button_tool;
        }
    }

    /// The active tool's swatches, selected swatch and palette, if it has colours.
    pub fn swatches_mut(&mut self) -> Option<(&mut [Color; 8], &mut usize, &'static [Color; 30])> {
        match self.tool {
            // Text takes the pen's swatches and palette.
            MarkupTool::Pen | MarkupTool::Text | MarkupTool::Stamp => {
                Some((&mut self.pen_swatches, &mut self.pen_swatch, &PALETTE))
            }
            MarkupTool::Highlighter => Some((
                &mut self.highlighter_swatches,
                &mut self.highlighter_swatch,
                &HIGHLIGHTER_PALETTE,
            )),
            MarkupTool::Eraser | MarkupTool::Select => None,
        }
    }

    /// The nib a new stroke of `tool` gets.
    pub fn nib(&self, tool: Tool) -> Nib {
        if tool == Tool::Highlighter && !self.highlighter_round {
            Nib::Chisel
        } else {
            Nib::Round
        }
    }

    /// The active layer of `score`: the chosen one if it still exists, else the top layer.
    pub fn layer(&self, score: &Score) -> Option<LayerId> {
        self.layers
            .get(&score.id)
            .filter(|id| score.layer(**id).is_some())
            .copied()
            .or_else(|| score.layers.last().map(|l| l.id))
    }

    /// What `annotate.state` returns.
    pub fn to_json(&self, active_layer: Option<LayerId>) -> Value {
        let style = |s: ToolStyle, tool: Tool| {
            let r = ToolStyle::widths(tool);
            json!({ "color": s.color.to_string(), "width": s.width, "widths": [r.min, r.max] })
        };
        json!({
            "tool": self.tool.id(),
            "pen": style(self.pen, Tool::Pen),
            "highlighter": style(self.highlighter, Tool::Highlighter),
            "whiteout": style(self.whiteout, Tool::Whiteout),
            "whiteoutOn": self.whiteout_on,
            "eraser": {
                "mode": self.eraser.id(),
                "width": self.eraser_width,
                "widths": [ERASER_WIDTHS.min, ERASER_WIDTHS.max],
            },
            "penOnly": self.pen_only,
            "drawWithTouch": !self.pen_only,
            "mode": self.mode,
            "touchLock": self.touch_lock,
            "penSwatches": hex(&self.pen_swatches),
            "penSwatch": self.pen_swatch + 1,
            "highlighterSwatch": self.highlighter_swatch + 1,
            "highlighterSwatches": hex(&self.highlighter_swatches),
            "penPalette": hex(&PALETTE),
            "highlighterPalette": hex(&HIGHLIGHTER_PALETTE),
            // The active tool's, as the toolbar shows them; none for the eraser.
            "swatches": match self.tool {
                MarkupTool::Pen | MarkupTool::Text | MarkupTool::Stamp => Some(hex(&self.pen_swatches)),
                MarkupTool::Highlighter => Some(hex(&self.highlighter_swatches)),
                MarkupTool::Eraser | MarkupTool::Select => None,
            },
            "swatch": match self.tool {
                MarkupTool::Pen | MarkupTool::Text | MarkupTool::Stamp => Some(self.pen_swatch + 1),
                MarkupTool::Highlighter => Some(self.highlighter_swatch + 1),
                MarkupTool::Eraser | MarkupTool::Select => None,
            },
            "palette": match self.tool {
                MarkupTool::Pen | MarkupTool::Text | MarkupTool::Stamp => Some(hex(&PALETTE)),
                MarkupTool::Highlighter => Some(hex(&HIGHLIGHTER_PALETTE)),
                MarkupTool::Eraser | MarkupTool::Select => None,
            },
            "highlighterRoundEnds": self.highlighter_round,
            "shapes": self.shapes,
            "penButton": self.button_tool.id(),
            "selection": self.selection.as_ref().map(|sel| json!({
                "filePage": u64::from(sel.page.0) + 1,
                "rect": [sel.rect.x0, sel.rect.y0, sel.rect.x1, sel.rect.y1],
                "annotations": sel.ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
            })),
            "text": {
                "size": self.text_size,
                "sizes": [TEXT_SIZES.min, TEXT_SIZES.max],
                "color": self.text_color.to_string(),
            },
            "stamp": {
                "stamp": self.stamp,
                "stamps": STAMPS,
                "size": self.stamp_size,
                "sizes": [STAMP_SIZES.min, STAMP_SIZES.max],
            },
            // Draw with Touch reads "Type with Touch" while the text tool is active.
            "touchLabel": if self.tool == MarkupTool::Text { "Type with Touch" } else { "Draw with Touch" },
            "ruler": self.ruler.map(|y| json!({
                "y": y,
                "halfHeight": RULER_HALF_HEIGHT,
                "reach": RULER_REACH,
            })),
            "layer": active_layer.map(|l| l.to_string()),
            "presets": self.presets.iter().map(|p| json!({
                "name": p.name,
                "tool": match p.tool {
                    Tool::Pen => "pen",
                    Tool::Highlighter => "highlighter",
                    Tool::Whiteout => "whiteout",
                },
                "color": p.color.to_string(),
                "width": p.width,
            })).collect::<Vec<_>>(),
        })
    }
}

fn hex(colors: &[Color]) -> Vec<String> {
    colors.iter().map(Color::to_string).collect()
}

/// Half the ruler band's height, in viewport pixels: its two long edges lie this far above and
/// below its centre.
pub const RULER_HALF_HEIGHT: f64 = 30.0;
/// A stroke starting this close to a ruler edge, in viewport pixels, is ruled along it.
pub const RULER_REACH: f64 = 18.0;

/// `points` (viewport pixels) ruled along whichever edge of the ruler centred at `y` the stroke
/// starts within [`RULER_REACH`] of; `None` when it starts further from both (spec ANNO-17).
pub fn rule_on_screen(
    y: f64,
    points: &[scorecraft_doc::InkPoint],
) -> Option<Vec<scorecraft_doc::InkPoint>> {
    let first = points.first()?;
    let at = Point::new(f64::from(first.x), f64::from(first.y));
    [y - RULER_HALF_HEIGHT, y + RULER_HALF_HEIGHT]
        .into_iter()
        .filter_map(|edge| Ruler::new(Point::new(0.0, edge), 0.0))
        .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
        .and_then(|edge| edge.rule(points, RULER_REACH))
}

/// Fingerprint of what ink on `page` looks like: every visible marking's id, layer and stamp in
/// drawing order (an edit always restamps), and which layers are visible.
fn ink_key(score: &Score, page: PageKey) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for layer in score.layers.iter().filter(|l| l.visible) {
        layer.id.hash(&mut h);
        for a in score
            .annotations_on(page)
            .iter()
            .filter(|a| a.layer == layer.id && is_drawn(&a.kind))
        {
            a.id.hash(&mut h);
            a.stamp.hash(&mut h);
        }
    }
    h.finish()
}

/// Tells the page cache what ink each of `file_pages` carries, so it draws ink on them (in the
/// background for pages a turn will show) and a turn never waits on it. Pages whose ink hasn't
/// changed cost a fingerprint only.
pub(crate) fn sync_ink(
    view: &ScoreView,
    score: Option<&Score>,
    curve: PressureCurve,
    file_pages: impl IntoIterator<Item = usize>,
) {
    for file_page in file_pages {
        let Ok(p) = u32::try_from(file_page) else {
            continue;
        };
        let page = PageKey(p);
        let marks: Vec<AnnotationKind> = match score {
            Some(s) => visible_marks(&s.layers, s.annotations_on(page))
                .into_iter()
                .cloned()
                .collect(),
            None => Vec::new(),
        };
        if marks.is_empty() {
            if view.ink_key(file_page).is_some() {
                view.set_ink(file_page, None);
            }
            continue;
        }
        let Some(score) = score else { continue };
        let key = ink_key(score, page);
        if view.ink_key(file_page) != Some(key) {
            view.set_ink(
                file_page,
                Some(PageInk {
                    key,
                    marks: Arc::new(marks),
                    curve,
                }),
            );
        }
    }
}

/// The library score the open score was opened from.
pub(crate) fn library_score(lib: Option<&Library>, id: Option<ScoreId>) -> Option<&Score> {
    lib.and_then(|l| l.scores.get(&id?))
}

/// Where a reader slot shows a page, for mapping viewport points onto it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageHit {
    /// 0-based score page in the reader.
    pub page: usize,
    pub file_page: usize,
    rotation: Rotation,
    src: Rect,
    dst: Rect,
    page_box: PageBox,
}

impl PageHit {
    /// A viewport point mapped into the page's PDF space. Points outside the slot extend its
    /// mapping, so ink can run into the margin.
    pub fn to_page(self, x: f64, y: f64) -> Option<Point> {
        let (dw, dh) = (self.dst.width(), self.dst.height());
        if !(dw > 0.0 && dh > 0.0) {
            return None;
        }
        let u = self.src.x0 + (x - self.dst.x0) / dw * self.src.width();
        let v = self.src.y0 + (y - self.dst.y0) / dh * self.src.height();
        let (fu, fv) = unturn(self.rotation, u, v);
        let size = self.page_box.display_size_pt();
        let p = self
            .page_box
            .display_to_pdf(1.0, Point::new(fu * size.width, fv * size.height))?;
        p.is_finite().then_some(p)
    }
}

impl crate::Session {
    /// Page points per viewport pixel where (`x`, `y`) lands on a page: what the shell needs to
    /// draw a stroke in progress at the width it will have. `None` off the pages.
    pub fn points_per_pixel(&self, x: f64, y: f64) -> Option<f64> {
        let open = self.score.as_ref()?;
        let h = hit(&open.reader, open.view.document(), x, y)?;
        let d = h.to_page(x, y)?.distance(h.to_page(x + 1.0, y)?);
        (d.is_finite() && d > 0.0).then_some(d)
    }
}

/// The page under viewport point (`x`, `y`), if any.
pub(crate) fn hit(reader: &ReaderState, doc: &Document, x: f64, y: f64) -> Option<PageHit> {
    let frame = reader.frame();
    let slot = frame
        .slots
        .iter()
        .find(|s| x >= s.dst.x0 && x <= s.dst.x1 && y >= s.dst.y0 && y <= s.dst.y1)?;
    let shown = reader.shown.get(slot.page)?;
    Some(PageHit {
        page: slot.page,
        file_page: shown.file_page,
        rotation: shown.rotation,
        src: slot.src,
        dst: slot.dst,
        page_box: page_box(doc.page(shown.file_page).ok()?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_the_spec() {
        let m = Markup::default();
        assert_eq!(m.tool, MarkupTool::Pen);
        assert!(m.pen_only, "only a pen draws by default");
        assert_eq!(m.eraser, EraserMode::Stroke);
        let (tool, style) = m.drawing().unwrap();
        assert_eq!((tool, style.color), (Tool::Pen, Color::BLACK));
        let j = m.to_json(None);
        assert_eq!(j["swatches"].as_array().unwrap().len(), 8);
        assert_eq!(j["drawWithTouch"], false);
    }
}
