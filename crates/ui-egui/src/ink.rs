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

use egui::{Color32, Context, Event, Painter, Pos2, Rect, Shape, Stroke, TouchPhase, pos2, vec2};
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
    born: f64,
}

/// A text note being typed (ANNO-8).
#[derive(Debug, Clone, PartialEq)]
pub struct TextEntry {
    /// Reader-local pixels of its top-left corner.
    pub at: Pos2,
    pub text: String,
    pub device: PointerKind,
}

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
        if self.live.is_some() {
            ctx.request_repaint();
        }
        InkFrame {
            block_reader: self.block,
        }
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
                if self.live.is_some() || self.text.is_some() {
                    return;
                }
                let tool = state.get("tool").and_then(Value::as_str).unwrap_or("pen");
                let purpose = if ruler(state)
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
                        self.text = Some(TextEntry {
                            at: a,
                            text: String::new(),
                            device: live.kind,
                        });
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
                        if let Some(v) = note(error, r)
                            && v.get("drawn").and_then(Value::as_bool) == Some(true)
                            && tool != "eraser"
                            && let Some(fp) = v.get("filePage").and_then(Value::as_u64)
                            && let Some((pts, stroke)) = live_shape(&live, session, state)
                        {
                            self.pending.push(Pending {
                                file_page: usize::try_from(fp.saturating_sub(1)).unwrap_or(0),
                                texture: None,
                                shape: pts,
                                stroke,
                                born: now,
                            });
                        }
                    }
                }
            }
        }
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
                target.map_or_else(|| "→ hidden".to_owned(), |t| format!("→ p. {t}")),
                egui::FontId::proportional(11.0),
                blue,
            );
        }
        for p in &self.pending {
            painter.add(Shape::line(
                p.shape.iter().map(|q| *q + off).collect(),
                p.stroke,
            ));
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
                (Purpose::Tool, _) => {
                    if let Some((pts, stroke)) = live_shape(live, session, state) {
                        let last = pts.last().copied();
                        painter.add(Shape::line(pts.iter().map(|q| *q + off).collect(), stroke));
                        if tool == "eraser"
                            && let Some(at) = last
                        {
                            painter.circle_stroke(
                                at + off,
                                (stroke.width * 4.0).max(4.0),
                                Stroke::new(1.0, Color32::GRAY),
                            );
                        }
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
                (Purpose::Ruler, _) => {}
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
        if let Some(t) = &self.text {
            // The new note's red box with its handle on the left (screenshot 29).
            let red = Color32::from_rgb(0xd0, 0x30, 0x30);
            let at = t.at + off;
            let size = state
                .pointer("/text/size")
                .and_then(Value::as_f64)
                .unwrap_or(12.0) as f32;
            let px = session
                .points_per_pixel(f64::from(t.at.x), f64::from(t.at.y))
                .map_or(1.0, |d| (1.0 / d) as f32);
            let h = size * 1.5 * px;
            let w = (t.text.chars().count().max(4) as f32 * size * 0.55 * px).min(rect.width());
            let r = Rect::from_min_size(at, vec2(w, h));
            painter.rect_stroke(r, 0.0, Stroke::new(1.5, red), egui::StrokeKind::Outside);
            let grip = Rect::from_center_size(pos2(r.left() - 8.0, r.center().y), vec2(8.0, h));
            painter.rect_filled(grip, 2.0, red);
        }
    }
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
