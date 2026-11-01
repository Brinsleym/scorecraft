//! Markup in the reader (spec `docs/spec/ink.md`): pens, touch and the mouse go through the palm
//! filter (ANNO-14) to the active markup tool; the stroke in progress is drawn live and kept on
//! screen until its page's texture shows it; the ruler band (ANNO-17), the selection rectangle
//! (ANNO-12) and a new text note's box (ANNO-8) are drawn over the pages. Every change goes
//! through an engine command; this module keeps only what is on screen.
//!
//! **Telling a pen from a finger.** winit 0.30 reports pens and fingers alike as touches. On
//! Windows (Windows Ink) a pen's touch carries a pressure and a finger's normally doesn't, so a
//! touch with a force is taken as a pen. Where that guess is wrong, *Draw with Touch* lets every
//! pointer draw.

use std::collections::HashMap;

use egui::{
    Color32, ColorImage, Context, Event, Painter, Pos2, Rect, Shape, Stroke, TextureHandle,
    TextureOptions, TouchPhase, Vec2, pos2, vec2,
};
use scorecraft_engine::Session;
use scorecraft_input::{PalmFilter, Phase, PointerKind};
use serde_json::{Value, json};

/// The id the mouse's gestures go by (touch ids come from the platform).
const MOUSE_ID: u64 = u64::MAX;
/// The engine's limit on samples per stroke; longer strokes are sent in pieces.
const MAX_SAMPLES: usize = 9_999;
/// A gesture that moves less than this many pixels is a tap.
const TAP_TRAVEL: f32 = 8.0;
/// How long a committed stroke stays drawn by the shell while its page re-renders.
const PENDING_SECS: f64 = 2.0;

/// The engine's ruler band: half its height and the reach of its edges (`annotate.state`).
fn ruler(state: &Value) -> Option<(f32, f32, f32)> {
    let r = state.get("ruler")?;
    Some((
        r.get("y")?.as_f64()? as f32,
        r.get("halfHeight")?.as_f64()? as f32,
        r.get("reach")?.as_f64()? as f32,
    ))
}

/// A `#rrggbb` or `#rrggbbaa` colour.
pub fn parse_color(hex: &str) -> Option<Color32> {
    let h = hex.strip_prefix('#')?;
    let byte = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok());
    match h.len() {
        6 => Some(Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?)),
        8 => Some(Color32::from_rgba_unmultiplied(
            byte(0)?,
            byte(2)?,
            byte(4)?,
            byte(6)?,
        )),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Purpose {
    /// The active tool's gesture.
    Tool,
    /// Dragging the ruler up or down.
    Ruler,
    /// Dragging the selection to move (or copy) it.
    MoveSelection,
    /// Dragging the selected text note to move it; a tap on it edits it.
    MoveNote,
}

#[derive(Debug, Clone)]
struct Live {
    id: u64,
    kind: PointerKind,
    purpose: Purpose,
    /// Reader-local pixels, pressure, milliseconds since the gesture began.
    points: Vec<[f64; 4]>,
    start: f64,
}

impl Live {
    fn first(&self) -> Pos2 {
        self.points
            .first()
            .map_or(Pos2::ZERO, |p| pos2(p[0] as f32, p[1] as f32))
    }

    fn last(&self) -> Pos2 {
        self.points
            .last()
            .map_or(Pos2::ZERO, |p| pos2(p[0] as f32, p[1] as f32))
    }

    fn travel(&self) -> f32 {
        (self.last() - self.first()).length()
    }
}

/// A committed stroke the shell keeps drawing until its page's texture changes.
struct Pending {
    file_page: usize,
    texture: Option<usize>,
    shape: Vec<Pos2>,
    stroke: Stroke,
    /// A highlighter stroke is kept as its one-layer picture instead.
    image: Option<(TextureHandle, Rect)>,
    born: f64,
}

/// The highlighter stroke in progress, drawn as one flat layer like the page will draw it: a
/// coverage mask where overlapping parts of the stroke take the larger coverage, never the sum,
/// so the preview never darkens where the stroke crosses itself or at its joints.
struct HighlightPreview {
    /// Reader-local pixels of the mask's top-left corner, and its size.
    origin: Pos2,
    w: usize,
    h: usize,
    cov: Vec<u8>,
    /// Points already stamped into the mask.
    done: usize,
    radius: f32,
    color: Color32,
    /// The part of the mask the stroke has touched: min x, min y, max x, max y.
    used: Option<[usize; 4]>,
    tex: Option<TextureHandle>,
}

/// The highlighter's opacity in the preview, over the page.
const HIGHLIGHT_ALPHA: f32 = 0.45;
/// Largest preview mask side, in pixels.
const MAX_PREVIEW: usize = 4096;

impl HighlightPreview {
    fn new(area: Rect, radius: f32, color: Color32) -> HighlightPreview {
        let w = (area.width().max(1.0) as usize).min(MAX_PREVIEW);
        let h = (area.height().max(1.0) as usize).min(MAX_PREVIEW);
        HighlightPreview {
            origin: Pos2::ZERO,
            w,
            h,
            cov: vec![0; w * h],
            done: 0,
            radius: radius.max(0.5),
            color,
            used: None,
            tex: None,
        }
    }

    /// Stamps the stroke's new segments (reader-local points) into the mask.
    fn add(&mut self, points: &[Pos2]) -> bool {
        if points.len() <= self.done {
            return false;
        }
        let from = self.done.saturating_sub(1);
        let r = self.radius;
        for i in from..points.len() {
            let (Some(&a), Some(&b)) = (points.get(i), points.get(i + 1).or(points.get(i))) else {
                continue;
            };
            let a = a - self.origin.to_vec2();
            let b = b - self.origin.to_vec2();
            let x0 = (a.x.min(b.x) - r - 1.0).floor().max(0.0) as usize;
            let y0 = (a.y.min(b.y) - r - 1.0).floor().max(0.0) as usize;
            let x1 = ((a.x.max(b.x) + r + 1.0).ceil().max(0.0) as usize).min(self.w);
            let y1 = ((a.y.max(b.y) + r + 1.0).ceil().max(0.0) as usize).min(self.h);
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            let ab = b - a;
            let len2 = ab.length_sq();
            for y in y0..y1 {
                for x in x0..x1 {
                    let q = pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let t = if len2 > 0.0 {
                        ((q - a).dot(ab) / len2).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let d = (q - (a + ab * t)).length();
                    let c = ((r + 0.5 - d).clamp(0.0, 1.0) * 255.0) as u8;
                    if let Some(px) = self.cov.get_mut(y * self.w + x)
                        && c > *px
                    {
                        *px = c;
                    }
                }
            }
            self.used = Some(match self.used {
                None => [x0, y0, x1, y1],
                Some([a0, b0, a1, b1]) => [a0.min(x0), b0.min(y0), a1.max(x1), b1.max(y1)],
            });
        }
        self.done = points.len();
        true
    }

    /// Uploads the touched part of the mask; returns where to draw it (reader-local).
    fn upload(&mut self, ctx: &Context) -> Option<Rect> {
        let [x0, y0, x1, y1] = self.used?;
        let (w, h) = (x1 - x0, y1 - y0);
        let [r, g, b, _] = self.color.to_srgba_unmultiplied();
        let mut px = Vec::with_capacity(w * h);
        for y in y0..y1 {
            for x in x0..x1 {
                let c = self.cov.get(y * self.w + x).copied().unwrap_or(0);
                let a = (f32::from(c) * HIGHLIGHT_ALPHA) as u8;
                px.push(Color32::from_rgba_unmultiplied(r, g, b, a));
            }
        }
        let image = ColorImage::new([w, h], px);
        match &mut self.tex {
            Some(t) => t.set(image, TextureOptions::LINEAR),
            None => {
                self.tex =
                    Some(ctx.load_texture("highlighter-preview", image, TextureOptions::LINEAR));
            }
        }
        Some(Rect::from_min_size(
            self.origin + vec2(x0 as f32, y0 as f32),
            vec2(w as f32, h as f32),
        ))
    }
}

/// A text note being typed into its red box on the page (ANNO-8).
#[derive(Debug, Clone, PartialEq)]
pub struct TextEntry {
    /// Reader-local pixels of its top-left corner.
    pub at: Pos2,
    pub text: String,
    pub device: PointerKind,
    /// The note being changed; `None` for a new one.
    pub note: Option<String>,
    /// Size in points, and its colour.
    pub size: f32,
    pub color: Color32,
    /// The box's size as the shell last laid its text out; `None` until it has.
    pub measured: Option<Vec2>,
    /// A new note's score page (1-based) and page-space corner, so it lands where it was put
    /// even if the page turns or scrolls before it is kept.
    pub anchor: Option<(usize, f64, f64)>,
}

/// What the reader looked like last frame: a change (a turn, scroll, zoom or pan) makes
/// everything the shell drew in screen pixels stale.
#[derive(Debug, Clone, PartialEq)]
struct ViewKey(Value);

/// What the reader should do with its own pointer handling this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InkFrame {
    /// Markup owns the pointer: no taps, swipes, pans or pinches for the reader.
    pub block_reader: bool,
}

#[derive(Default)]
pub struct InkCanvas {
    palm: PalmFilter,
    /// Touches down now.
    touches: HashMap<u64, PointerKind>,
    live: Option<Live>,
    pending: Vec<Pending>,
    /// The reader stays blocked until every pointer lifts, then one frame more (its click is
    /// read on the frame the pointer lifts).
    block: bool,
    release: bool,
    /// The note being typed, if any.
    pub text: Option<TextEntry>,
    /// The text note selected (its red box shows it can be moved), by id.
    pub note: Option<String>,
    /// The highlighter stroke in progress.
    preview: Option<HighlightPreview>,
    /// Where the preview is drawn this frame.
    preview_rect: Option<Rect>,
    view: Option<ViewKey>,
    /// The pointer whose press ended typing: the rest of its gesture does nothing.
    swallow: Option<u64>,
    /// The last selection rectangle, reader-local pixels.
    pub selection: Option<Rect>,
    /// The next selection drag copies instead of moving.
    pub copy_next: bool,
    /// The next tap places a snip of the selection rectangle there (PAGE-7).
    pub snip_armed: bool,
    /// The page a shortcut from the selection rectangle jumps to (NAV-1), 1-based.
    pub shortcut_target: u32,
}

impl InkCanvas {
    pub fn new() -> InkCanvas {
        InkCanvas::default()
    }

    /// Forgets everything on screen (a new score, or markup turned off).
    pub fn reset(&mut self) {
        *self = InkCanvas::default();
    }

    /// Whether markup is handling a gesture now.
    pub fn busy(&self) -> bool {
        self.live.is_some() || self.text.is_some()
    }

    /// One frame of markup input. `state` is `annotate.state`; `rect` the reader's area.
    /// Errors from the engine go to `error`.
    pub fn input(
        &mut self,
        ctx: &Context,
        rect: Rect,
        session: &mut Session,
        state: &Value,
        error: &mut Option<String>,
    ) -> InkFrame {
        if self.release {
            self.block = false;
            self.release = false;
        }
        if state.get("mode").and_then(Value::as_bool) != Some(true) {
            if self.live.is_some() || !self.touches.is_empty() || self.selection.is_some() {
                self.reset();
            }
            return InkFrame::default();
        }
        self.follow_view(session);
        let pen_only = state.get("penOnly").and_then(Value::as_bool) != Some(false);
        let touch_lock = state.get("touchLock").and_then(Value::as_bool) == Some(true);
        let (events, now) = ctx.input(|i| (i.events.clone(), i.time));
        let now_ms = (now * 1000.0).max(0.0) as u64;
        // egui also turns the first touch into mouse events; those are not the mouse.
        let touch_frame = events.iter().any(|e| matches!(e, Event::Touch { .. }));
        for e in events {
            match e {
                Event::Touch {
                    id,
                    phase,
                    pos,
                    force,
                    ..
                } => {
                    let kind = if force.is_some() {
                        PointerKind::Pen
                    } else {
                        PointerKind::Touch
                    };
                    let phase = match phase {
                        TouchPhase::Start => Phase::Down,
                        TouchPhase::Move => Phase::Move,
                        TouchPhase::End => Phase::Up,
                        TouchPhase::Cancel => Phase::Cancel,
                    };
                    let tid = id.0;
                    let f = self.palm.event(tid, kind, phase, now_ms);
                    for c in f.cancel {
                        // A touch gesture cut short by the pen: its ink and its tap go.
                        if self.live.as_ref().is_some_and(|l| l.id == c) {
                            self.live = None;
                        }
                        self.block = true;
                    }
                    match phase {
                        Phase::Down => {
                            self.touches.insert(tid, kind);
                        }
                        Phase::Up | Phase::Cancel => {
                            self.touches.remove(&tid);
                        }
                        Phase::Move => {}
                    }
                    if phase == Phase::Cancel {
                        if self.live.as_ref().is_some_and(|l| l.id == tid) {
                            self.live = None;
                        }
                        self.block = true;
                        continue;
                    }
                    if !f.pass {
                        self.block = true;
                        continue;
                    }
                    if kind == PointerKind::Touch && touch_lock {
                        self.block = true;
                        continue;
                    }
                    if kind == PointerKind::Touch && pen_only && !self.on_ruler(state, pos, rect) {
                        // A finger turns, pans and zooms as outside markup mode.
                        continue;
                    }
                    self.block = true;
                    let p = pos - rect.min;
                    self.feed(
                        tid,
                        kind,
                        phase,
                        pos2(p.x, p.y),
                        force,
                        now,
                        session,
                        state,
                        error,
                    );
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } if !touch_frame && self.touches.is_empty() => {
                    let inside = rect.contains(pos);
                    let mine = self.live.as_ref().is_some_and(|l| l.id == MOUSE_ID);
                    if pen_only && !(inside && self.on_ruler(state, pos, rect)) && !mine {
                        continue;
                    }
                    if pressed && !inside {
                        continue;
                    }
                    self.block = true;
                    let phase = if pressed { Phase::Down } else { Phase::Up };
                    let p = pos - rect.min;
                    self.feed(
                        MOUSE_ID,
                        PointerKind::Mouse,
                        phase,
                        pos2(p.x, p.y),
                        None,
                        now,
                        session,
                        state,
                        error,
                    );
                }
                Event::PointerMoved(pos)
                    if !touch_frame && self.live.as_ref().is_some_and(|l| l.id == MOUSE_ID) =>
                {
                    let p = pos - rect.min;
                    self.feed(
                        MOUSE_ID,
                        PointerKind::Mouse,
                        Phase::Move,
                        pos2(p.x, p.y),
                        None,
                        now,
                        session,
                        state,
                        error,
                    );
                }
                _ => {}
            }
        }
        let anything_down = ctx.input(|i| i.pointer.any_down()) || !self.touches.is_empty();
        if self.block && self.live.is_none() && !anything_down {
            self.release = true;
        }
        self.update_preview(ctx, rect, session, state);
        if self.live.is_some() {
            ctx.request_repaint();
        }
        InkFrame {
            block_reader: self.block,
        }
    }

    /// Forgets what was drawn in screen pixels when the view moved under it (a turn, the wheel,
    /// a zoom or a pan): kept strokes, the selection rectangle and the highlighter preview would
    /// otherwise stay where they were while the page moved (the owner's "scroll breaks all
    /// markup").
    fn follow_view(&mut self, session: &mut Session) {
        let key = session.reader_state().map(|st| {
            ViewKey(json!([
                st.get("onScreen"),
                st.get("half"),
                st.get("zoom"),
                st.get("viewport"),
                st.get("layout"),
            ]))
        });
        if key == self.view {
            return;
        }
        let moved = self.view.is_some();
        self.view = key;
        if !moved {
            return;
        }
        self.pending.clear();
        self.preview = None;
        self.preview_rect = None;
        if self.selection.take().is_some() {
            let _ = session.dispatch("annotate.select.clear", &json!({}));
        }
        self.snip_armed = false;
        // A note being typed is kept where it was put (the page moved, not the note).
        if self.text.is_some() {
            let mut ignored = None;
            self.commit_text(session, &mut ignored);
        }
        // A stroke cut by a turn is dropped rather than drawn across two pages.
        if self
            .live
            .as_ref()
            .is_some_and(|l| l.purpose != Purpose::Ruler)
        {
            self.live = None;
        }
    }

    /// Stamps the live highlighter stroke into its preview and uploads it.
    fn update_preview(&mut self, ctx: &Context, rect: Rect, session: &Session, state: &Value) {
        let Some(live) = self
            .live
            .as_ref()
            .filter(|l| l.purpose == Purpose::Tool)
            .filter(|_| state.get("tool").and_then(Value::as_str) == Some("highlighter"))
        else {
            self.preview = None;
            self.preview_rect = None;
            return;
        };
        if self.preview.is_none() {
            let Some((_, stroke)) = live_shape(live, session, state) else {
                return;
            };
            self.preview = Some(HighlightPreview::new(
                rect,
                stroke.width / 2.0,
                stroke.color,
            ));
        }
        let points: Vec<Pos2> = live
            .points
            .iter()
            .map(|p| pos2(p[0] as f32, p[1] as f32))
            .collect();
        if let Some(pv) = self.preview.as_mut()
            && pv.add(&points)
        {
            self.preview_rect = pv.upload(ctx);
        }
    }

    /// The text notes on screen, reader-local.
    fn notes(session: &Session) -> Vec<(String, Rect, scorecraft_engine::NoteOnScreen)> {
        session
            .notes_on_screen()
            .into_iter()
            .map(|n| {
                let [x0, y0, x1, y1] = n.rect;
                let r = Rect::from_min_max(pos2(x0 as f32, y0 as f32), pos2(x1 as f32, y1 as f32));
                (n.id.clone(), r, n)
            })
            .collect()
    }

    /// The selected note's rectangle on screen, reader-local.
    pub fn note_rect(&self, session: &Session) -> Option<Rect> {
        self.note_rect_of(session, self.note.as_deref())
    }

    /// A note's rectangle on screen, reader-local, by id.
    pub fn note_rect_of(&self, session: &Session, id: Option<&str>) -> Option<Rect> {
        let id = id?;
        Self::notes(session)
            .into_iter()
            .find(|n| n.0 == id)
            .map(|n| n.1)
    }

    /// Ends typing: adds the new note or changes the one being edited. A new note stays
    /// selected; blank text adds nothing (and deletes an edited note).
    pub fn commit_text(&mut self, session: &mut Session, error: &mut Option<String>) {
        let Some(entry) = self.text.take() else {
            return;
        };
        match &entry.note {
            None if entry.text.trim().is_empty() => {}
            None => {
                let p = match entry.anchor {
                    Some((page, x, y)) => json!({
                        "text": entry.text, "x": x, "y": y, "space": "page", "page": page,
                        "device": entry.device.id(),
                    }),
                    None => json!({
                        "text": entry.text, "x": entry.at.x, "y": entry.at.y,
                        "device": entry.device.id(),
                    }),
                };
                let r = session.dispatch("annotate.text.add", &p);
                self.note = note(error, r).and_then(|v| {
                    v.get("annotation")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
            }
            Some(id) => {
                let r = session.dispatch(
                    "annotate.text.edit",
                    &json!({ "annotation": id, "text": entry.text }),
                );
                let deleted = note(error, r)
                    .is_some_and(|v| v.get("deleted").and_then(Value::as_bool) == Some(true));
                self.note = (!deleted).then(|| id.clone());
            }
        }
    }

    /// Where a new note's box goes for a tap at `at`: there, or moved down below any note it
    /// would cover (a new note never starts on top of another; it can be moved over one after).
    fn free_spot(at: Pos2, size: Vec2, notes: &[Rect]) -> Pos2 {
        let mut at = at;
        for _ in 0..=notes.len() {
            let r = Rect::from_min_size(at, size);
            match notes.iter().find(|n| n.intersects(r)) {
                Some(n) => at.y = n.bottom() + 2.0,
                None => break,
            }
        }
        at
    }

    /// Whether `pos` (screen) is on the ruler's body, where a drag moves it.
    fn on_ruler(&self, state: &Value, pos: Pos2, rect: Rect) -> bool {
        ruler(state).is_some_and(|(y, half, reach)| {
            ((pos.y - rect.min.y) - y).abs() < (half - reach).max(4.0)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn feed(
        &mut self,
        id: u64,
        kind: PointerKind,
        phase: Phase,
        at: Pos2,
        force: Option<f32>,
        now: f64,
        session: &mut Session,
        state: &Value,
        error: &mut Option<String>,
    ) {
        let pressure = f64::from(force.unwrap_or(1.0).clamp(0.0, 1.0));
        match phase {
            Phase::Down => {
                if self.live.is_some() {
                    return;
                }
                let tool = state.get("tool").and_then(Value::as_str).unwrap_or("pen");
                if let Some(entry) = &self.text {
                    // A tap in the box being typed into belongs to the text field; anywhere else
                    // ends typing (and is not a new note).
                    if entry_rect(entry, session).expand(12.0).contains(at) {
                        return;
                    }
                    self.commit_text(session, error);
                    self.note = None;
                    self.swallow = Some(id);
                    return;
                }
                // With the text tool a press on a note (or the selected one's handle) selects
                // it: a drag moves it, a tap opens it for typing.
                if tool == "text"
                    && !self
                        .note_rect(session)
                        .is_some_and(|r| r.expand2(vec2(14.0, 4.0)).contains(at))
                    && let Some((hit, _, _)) =
                        Self::notes(session).into_iter().find(|n| n.1.contains(at))
                {
                    self.note = Some(hit);
                }
                let on_note = tool == "text"
                    && self
                        .note_rect(session)
                        .is_some_and(|r| r.expand2(vec2(14.0, 4.0)).contains(at));
                let purpose = if on_note {
                    Purpose::MoveNote
                } else if ruler(state)
                    .is_some_and(|(y, half, reach)| (at.y - y).abs() < (half - reach).max(4.0))
                {
                    Purpose::Ruler
                } else if tool == "select" && self.selection.is_some_and(|r| r.contains(at)) {
                    Purpose::MoveSelection
                } else {
                    Purpose::Tool
                };
                self.live = Some(Live {
                    id,
                    kind,
                    purpose,
                    points: vec![[f64::from(at.x), f64::from(at.y), pressure, 0.0]],
                    start: now,
                });
            }
            Phase::Move | Phase::Up => {
                if self.swallow == Some(id) {
                    if phase == Phase::Up {
                        self.swallow = None;
                    }
                    return;
                }
                let Some(live) = self.live.as_mut().filter(|l| l.id == id) else {
                    return;
                };
                let t = ((now - live.start) * 1000.0).max(0.0);
                live.points
                    .push([f64::from(at.x), f64::from(at.y), pressure, t]);
                if live.purpose == Purpose::Ruler {
                    let r = session.dispatch("annotate.ruler.set", &json!({ "y": at.y }));
                    note(error, r);
                }
                // A very long stroke is committed in pieces, each continuing the last.
                if live.purpose == Purpose::Tool && live.points.len() >= MAX_SAMPLES {
                    let last = live.points.last().copied();
                    if let Some(done) = self.live.take() {
                        self.finish(done, session, state, now, error);
                    }
                    if let Some(p) = last {
                        self.live = Some(Live {
                            id,
                            kind,
                            purpose: Purpose::Tool,
                            points: vec![[p[0], p[1], p[2], 0.0]],
                            start: now,
                        });
                    }
                }
                if phase == Phase::Up
                    && let Some(done) = self.live.take()
                {
                    self.finish(done, session, state, now, error);
                }
            }
            Phase::Cancel => {
                if self.live.as_ref().is_some_and(|l| l.id == id) {
                    self.live = None;
                }
            }
        }
    }

    /// The gesture is over: apply it.
    fn finish(
        &mut self,
        live: Live,
        session: &mut Session,
        state: &Value,
        now: f64,
        error: &mut Option<String>,
    ) {
        let device = live.kind.id();
        let (a, b) = (live.first(), live.last());
        match live.purpose {
            Purpose::Ruler => {}
            Purpose::MoveNote => {
                let (Some(id), Some(r)) = (self.note.clone(), self.note_rect(session)) else {
                    return;
                };
                if live.travel() >= TAP_TRAVEL {
                    let to = r.min + (b - a);
                    let res = session.dispatch(
                        "annotate.text.edit",
                        &json!({ "annotation": id, "x": to.x, "y": to.y }),
                    );
                    note(error, res);
                } else {
                    self.edit_note(session, &id);
                }
            }
            Purpose::MoveSelection => {
                let (Some(sel), true) = (self.selection, live.travel() >= TAP_TRAVEL) else {
                    return;
                };
                let d = b - a;
                let to = sel.center() + d;
                let id = if self.copy_next {
                    "annotate.select.copy"
                } else {
                    "annotate.select.move"
                };
                let r = session.dispatch(id, &json!({ "x": to.x, "y": to.y }));
                if note(error, r).is_some() {
                    self.selection = Some(sel.translate(d));
                    self.copy_next = false;
                }
            }
            Purpose::Tool => {
                let tool = state.get("tool").and_then(Value::as_str).unwrap_or("pen");
                match tool {
                    "text" => {
                        let notes = Self::notes(session);
                        if let Some((id, _, _)) = notes.iter().find(|n| n.1.contains(a)) {
                            // A tap on a note selects it and opens it for typing.
                            self.note = Some(id.clone());
                            let id = id.clone();
                            self.edit_note(session, &id);
                        } else if self.note.take().is_some() {
                            // The first tap away from a selected note just lets it go.
                        } else {
                            let size = state
                                .pointer("/text/size")
                                .and_then(Value::as_f64)
                                .unwrap_or(12.0) as f32;
                            let color = state
                                .pointer("/text/color")
                                .and_then(Value::as_str)
                                .and_then(parse_color)
                                .unwrap_or(Color32::BLACK);
                            let mut entry = TextEntry {
                                at: a,
                                text: String::new(),
                                device: live.kind,
                                note: None,
                                size,
                                color,
                                measured: None,
                                anchor: None,
                            };
                            let rects: Vec<Rect> = notes.iter().map(|n| n.1).collect();
                            let box_size = entry_rect(&entry, session).size();
                            entry.at = Self::free_spot(a, box_size, &rects);
                            entry.anchor =
                                session.view_to_page(f64::from(entry.at.x), f64::from(entry.at.y));
                            self.text = Some(entry);
                        }
                    }
                    "stamp" => {
                        let mut p = json!({ "x": a.x, "y": a.y, "device": device });
                        if live.travel() >= TAP_TRAVEL * 2.0
                            && let Some(o) = p.as_object_mut()
                        {
                            o.insert("toX".into(), json!(b.x));
                            o.insert("toY".into(), json!(b.y));
                        }
                        note(error, session.dispatch("annotate.stamp.place", &p));
                    }
                    "select" if self.snip_armed && live.travel() < TAP_TRAVEL => {
                        if let Some(sel) = self.selection {
                            let r = session.dispatch(
                                "page.snip.create",
                                &json!({
                                    "x0": sel.min.x, "y0": sel.min.y,
                                    "x1": sel.max.x, "y1": sel.max.y,
                                    "toX": a.x, "toY": a.y,
                                }),
                            );
                            note(error, r);
                        }
                        self.snip_armed = false;
                        self.selection = None;
                        let _ = session.dispatch("annotate.select.clear", &json!({}));
                    }
                    "select" => {
                        let r = session.dispatch(
                            "annotate.select.rect",
                            &json!({ "x0": a.x, "y0": a.y, "x1": b.x, "y1": b.y }),
                        );
                        // An empty rectangle stays selected too: it can become a snip or a
                        // shortcut (ANNO-12).
                        self.selection = note(error, r)
                            .filter(|_| live.travel() >= TAP_TRAVEL)
                            .map(|_| Rect::from_two_pos(a, b));
                        if self.selection.is_none() {
                            let _ = session.dispatch("annotate.select.clear", &json!({}));
                        }
                    }
                    _ => {
                        let points: Vec<Value> = live
                            .points
                            .iter()
                            .map(|p| json!([p[0], p[1], p[2], p[3]]))
                            .collect();
                        let r = session.dispatch(
                            "annotate.stroke",
                            &json!({ "points": points, "device": device }),
                        );
                        let image = if tool == "highlighter" {
                            self.preview_rect
                                .take()
                                .and_then(|r| self.preview.take()?.tex.map(|t| (t, r)))
                        } else {
                            None
                        };
                        if let Some(v) = note(error, r)
                            && v.get("drawn").and_then(Value::as_bool) == Some(true)
                            && tool != "eraser"
                            && let Some(fp) = v.get("filePage").and_then(Value::as_u64)
                            && let Some((pts, stroke)) = live_shape(&live, session, state)
                        {
                            let (shape, image) = match image {
                                Some(im) => (Vec::new(), Some(im)),
                                None => (pts, None),
                            };
                            self.pending.push(Pending {
                                file_page: usize::try_from(fp.saturating_sub(1)).unwrap_or(0),
                                texture: None,
                                shape,
                                stroke,
                                image,
                                born: now,
                            });
                        }
                    }
                }
            }
        }
    }

    /// Opens note `id` for typing in place.
    fn edit_note(&mut self, session: &Session, id: &str) {
        let Some((_, r, n)) = Self::notes(session).into_iter().find(|n| n.0 == id) else {
            return;
        };
        self.text = Some(TextEntry {
            at: r.min,
            text: n.text,
            device: PointerKind::Pen,
            note: Some(id.to_owned()),
            size: n.size,
            color: parse_color(&n.color).unwrap_or(Color32::BLACK),
            measured: None,
            anchor: None,
        });
    }

    /// Notes which texture each kept stroke's page had when it was committed, and drops the
    /// strokes whose page has been redrawn since (or that waited too long).
    pub fn settle(&mut self, now: f64, texture_of: &dyn Fn(usize) -> Option<usize>) {
        self.pending.retain_mut(|p| {
            let current = texture_of(p.file_page);
            match p.texture {
                None => {
                    p.texture = Some(current.unwrap_or(0));
                    true
                }
                Some(t) => current.unwrap_or(0) == t && now - p.born < PENDING_SECS,
            }
        });
    }

    /// Draws the live and kept strokes, the ruler and the selection over the pages.
    pub fn paint(&self, painter: &Painter, rect: Rect, session: &Session, state: &Value) {
        if state.get("mode").and_then(Value::as_bool) != Some(true) {
            return;
        }
        let off = rect.min.to_vec2();
        // Score Shortcuts show as faint outlines in markup mode only (NAV-1).
        let blue = Color32::from_rgb(0x30, 0x70, 0xe0);
        for ([x0, y0, x1, y1], target) in session.shortcut_rects() {
            let r = Rect::from_min_max(pos2(x0 as f32, y0 as f32), pos2(x1 as f32, y1 as f32))
                .translate(off);
            painter.rect_stroke(
                r,
                2.0,
                Stroke::new(1.0, blue.gamma_multiply(0.7)),
                egui::StrokeKind::Middle,
            );
            painter.text(
                r.left_top() + vec2(2.0, 1.0),
                egui::Align2::LEFT_TOP,
                target.map_or_else(|| "to a hidden page".to_owned(), |t| format!("to p. {t}")),
                egui::FontId::proportional(11.0),
                blue,
            );
        }
        let full_uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        for p in &self.pending {
            if let Some((t, r)) = &p.image {
                painter.image(t.id(), r.translate(off), full_uv, Color32::WHITE);
            } else {
                painter.add(Shape::line(
                    p.shape.iter().map(|q| *q + off).collect(),
                    p.stroke,
                ));
            }
        }
        if let Some(live) = &self.live {
            let tool = state.get("tool").and_then(Value::as_str).unwrap_or("pen");
            match (live.purpose, tool) {
                (Purpose::Tool, "select") => {
                    let r = Rect::from_two_pos(live.first(), live.last()).translate(off);
                    painter.rect_stroke(
                        r,
                        0.0,
                        Stroke::new(1.5, Color32::from_rgb(0xd0, 0x30, 0x30)),
                        egui::StrokeKind::Middle,
                    );
                }
                (Purpose::Tool, "stamp" | "text") => {}
                (Purpose::Tool, "highlighter") => {
                    // One flat layer, as the page will draw it (never darker where it overlaps).
                    if let (Some(pv), Some(r)) = (&self.preview, self.preview_rect)
                        && let Some(t) = &pv.tex
                    {
                        painter.image(t.id(), r.translate(off), full_uv, Color32::WHITE);
                    }
                }
                (Purpose::Tool, "eraser") => {
                    // Only where the eraser is: no trail behind it.
                    let state_w = state
                        .pointer("/eraser/width")
                        .and_then(Value::as_f64)
                        .unwrap_or(8.0) as f32;
                    let px = session
                        .points_per_pixel(f64::from(live.last().x), f64::from(live.last().y))
                        .map_or(1.0, |d| (1.0 / d) as f32);
                    painter.circle_stroke(
                        live.last() + off,
                        (state_w * px / 2.0).max(4.0),
                        Stroke::new(1.5, Color32::from_gray(110)),
                    );
                }
                (Purpose::Tool, _) => {
                    if let Some((pts, stroke)) = live_shape(live, session, state) {
                        painter.add(Shape::line(pts.iter().map(|q| *q + off).collect(), stroke));
                    }
                }
                (Purpose::MoveSelection, _) => {
                    if let Some(sel) = self.selection {
                        let d = live.last() - live.first();
                        painter.rect_stroke(
                            sel.translate(d + off),
                            0.0,
                            Stroke::new(1.5, Color32::from_rgb(0xd0, 0x30, 0x30)),
                            egui::StrokeKind::Middle,
                        );
                    }
                }
                (Purpose::Ruler | Purpose::MoveNote, _) => {}
            }
        }
        if let Some((y, half, _)) = ruler(state) {
            paint_ruler(painter, rect, rect.min.y + y, half);
        }
        if state.get("tool").and_then(Value::as_str) == Some("select")
            && let Some(sel) = self.selection
            && !self
                .live
                .as_ref()
                .is_some_and(|l| l.purpose == Purpose::MoveSelection)
        {
            let r = sel.translate(off);
            let red = Color32::from_rgb(0xd0, 0x30, 0x30);
            painter.rect_stroke(r, 0.0, Stroke::new(1.5, red), egui::StrokeKind::Middle);
            for c in [
                r.left_top(),
                r.right_top(),
                r.left_bottom(),
                r.right_bottom(),
            ] {
                painter.rect_filled(Rect::from_center_size(c, vec2(7.0, 7.0)), 1.0, red);
            }
        }
        // The selected note: its red box and handle show it can be moved (dragged by the box).
        if self.text.is_none()
            && state.get("tool").and_then(Value::as_str) == Some("text")
            && let Some(r) = self.note_rect(session)
        {
            let d = self
                .live
                .as_ref()
                .filter(|l| l.purpose == Purpose::MoveNote)
                .map_or(Vec2::ZERO, |l| l.last() - l.first());
            note_box(painter, r.translate(off + d));
        }
    }
}

/// A note's red box with its handle on the left (screenshot 29).
pub fn note_box(painter: &Painter, r: Rect) {
    let red = Color32::from_rgb(0xd0, 0x30, 0x30);
    painter.rect_stroke(r, 0.0, Stroke::new(1.5, red), egui::StrokeKind::Outside);
    let grip = Rect::from_center_size(pos2(r.left() - 8.0, r.center().y), vec2(8.0, r.height()));
    painter.rect_filled(grip, 2.0, red);
}

/// The box a note being typed takes, reader-local: as wide as its text (four characters at
/// least), as tall as its lines.
pub fn entry_rect(entry: &TextEntry, session: &Session) -> Rect {
    if let Some(size) = entry.measured {
        return Rect::from_min_size(entry.at, size);
    }
    let px = session
        .points_per_pixel(f64::from(entry.at.x), f64::from(entry.at.y))
        .map_or(1.0, |d| (1.0 / d) as f32);
    let size = entry.size * px;
    let lines = entry.text.split('\n').count().max(1) as f32;
    let longest = entry
        .text
        .split('\n')
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(4) as f32;
    Rect::from_min_size(
        entry.at,
        vec2(longest * size * 0.6 + 8.0, lines * size * 1.3 + 6.0),
    )
}

/// Records an engine error; returns the value on success.
fn note(
    error: &mut Option<String>,
    r: Result<Value, scorecraft_engine::EngineError>,
) -> Option<Value> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            *error = Some(e.to_string());
            None
        }
    }
}

/// The live stroke's points and pen, sized as it will be drawn.
fn live_shape(live: &Live, session: &Session, state: &Value) -> Option<(Vec<Pos2>, Stroke)> {
    let tool = state.get("tool").and_then(Value::as_str).unwrap_or("pen");
    let first = live.first();
    let px_per_pt = session
        .points_per_pixel(f64::from(first.x), f64::from(first.y))
        .map_or(1.0, |d| 1.0 / d) as f32;
    let (width, color) = match tool {
        "highlighter" => {
            let c = state
                .pointer("/highlighter/color")
                .and_then(Value::as_str)
                .and_then(parse_color)
                .unwrap_or(Color32::YELLOW);
            let w = state.pointer("/highlighter/width").and_then(Value::as_f64);
            (w, c.gamma_multiply(0.45))
        }
        "eraser" => (Some(1.0 / f64::from(px_per_pt)), Color32::from_gray(150)),
        _ if state.get("whiteoutOn").and_then(Value::as_bool) == Some(true) => (
            state.pointer("/whiteout/width").and_then(Value::as_f64),
            Color32::WHITE,
        ),
        _ => (
            state.pointer("/pen/width").and_then(Value::as_f64),
            state
                .pointer("/pen/color")
                .and_then(Value::as_str)
                .and_then(parse_color)
                .unwrap_or(Color32::BLACK),
        ),
    };
    let width = (width.unwrap_or(1.5) as f32 * px_per_pt).max(1.0);
    let pts = live
        .points
        .iter()
        .map(|p| pos2(p[0] as f32, p[1] as f32))
        .collect();
    Some((pts, Stroke::new(width, color)))
}

/// The ruler: a light band across the reader's whole width with ticks along both edges.
fn paint_ruler(painter: &Painter, rect: Rect, y: f32, half: f32) {
    let band = Rect::from_min_max(pos2(rect.left(), y - half), pos2(rect.right(), y + half));
    painter.rect_filled(
        band,
        0.0,
        Color32::from_rgba_unmultiplied(225, 232, 245, 200),
    );
    let edge = Stroke::new(1.0, Color32::from_gray(90));
    painter.line_segment([band.left_top(), band.right_top()], edge);
    painter.line_segment([band.left_bottom(), band.right_bottom()], edge);
    let mut x = rect.left();
    let mut k = 0u32;
    while x <= rect.right() {
        let len = if k.is_multiple_of(5) { 10.0 } else { 5.0 };
        painter.line_segment([pos2(x, band.top()), pos2(x, band.top() + len)], edge);
        painter.line_segment([pos2(x, band.bottom()), pos2(x, band.bottom() - len)], edge);
        x += 8.0;
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_parse() {
        assert_eq!(parse_color("#ff0000"), Some(Color32::from_rgb(255, 0, 0)));
        assert_eq!(
            parse_color("#00ff0080"),
            Some(Color32::from_rgba_unmultiplied(0, 255, 0, 128))
        );
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_color("#12"), None);
    }
}
