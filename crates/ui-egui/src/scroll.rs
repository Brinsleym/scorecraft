//! Smooth scrolling for the shell's vertical lists (the library grid and list, Lists,
//! Recently Deleted, Settings, a score's pages), with the scroll bar in the page's margin.
//!
//! The suite's egui already coasts after a touch drag (kinetic scrolling) and follows a
//! trackpad finger for finger. A mouse wheel moves in coarse notches, which egui spreads over
//! only a tenth of a second, so the page jumps. [`SmoothScroll`] takes a wheel's movement over
//! the area and eases it in over about a quarter of a second, as a browser does. Trackpads,
//! touch drags and the scroll bar itself are left to egui.
//!
//! With a [`SmoothScroll::gutter`], the bar sits in the page's right margin rather than over
//! the content: the content keeps its width, so the left and right margins stay the same, and
//! the bar is centred in the margin on the right.

use egui::{
    Color32, Event, Id, MouseWheelUnit, Rect, ScrollArea, Sense, Ui, UiBuilder, style::ScrollStyle,
};

/// How quickly a wheel notch is eased in: this fraction of it in [`EASE_SECONDS`].
const EASE_FRACTION: f32 = 0.9;
const EASE_SECONDS: f32 = 0.22;
/// Once this little (in points) is left to ease in, it is applied at once.
const SETTLE: f32 = 0.5;
/// After a wheel event, keep easing the wheel's movement for this long: egui keeps feeding a
/// notch in for about a tenth of a second after it arrives.
const WHEEL_HOLD: f64 = 0.25;
/// Point deltas at least this long come from a wheel, not a trackpad (egui's own rule, from
/// `WheelState::on_wheel_event`).
const WHEEL_POINTS: f32 = 8.0;
/// The widest scroll bar, in points; a narrower gutter gets half its width.
const BAR: f32 = 8.0;

/// What a [`SmoothScroll`] remembers between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Memory {
    /// The wheel movement still to ease in, in points (positive moves the content down).
    pending: f32,
    /// Ease the wheel's movement until this time.
    wheel_until: f64,
    /// Last frame: where the area was, its offset and its largest offset.
    rect: Option<Rect>,
    offset: f32,
    max_offset: f32,
}

/// A vertical [`ScrollArea`] with an eased mouse wheel, and optionally its bar in a gutter.
#[derive(Debug, Clone)]
pub struct SmoothScroll {
    id_salt: &'static str,
    gutter: Option<f32>,
    auto_shrink: bool,
    max_height: Option<f32>,
}

impl SmoothScroll {
    /// A vertical scroll area; `id_salt` tells it apart from others in the same `Ui`.
    pub fn vertical(id_salt: &'static str) -> Self {
        SmoothScroll {
            id_salt,
            gutter: None,
            auto_shrink: true,
            max_height: None,
        }
    }

    /// Put the scroll bar in the `width` points of margin to the right of the `Ui` it is shown
    /// in, centred, instead of over the content. The content keeps the `Ui`'s full width.
    pub fn gutter(mut self, width: f32) -> Self {
        self.gutter = (width > 0.0).then_some(width);
        self
    }

    /// As [`ScrollArea::auto_shrink`] for both directions.
    pub fn auto_shrink(mut self, shrink: bool) -> Self {
        self.auto_shrink = shrink;
        self
    }

    /// As [`ScrollArea::max_height`].
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
        let key = ui.make_persistent_id(("smooth-scroll", self.id_salt));
        let Some(gutter) = self.gutter else {
            return self.show_area(ui, key, None, add_contents);
        };
        // A child reaching `gutter` points past the right edge, into the page's margin; the
        // parent only advances by its own width, so its layout doesn't grow.
        let width = ui.available_width();
        let mut rect = ui.available_rect_before_wrap();
        rect.max.x += gutter;
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(*ui.layout()));
        let inner = self.show_area(&mut child, key, Some((gutter, width)), add_contents);
        let used = child.min_rect();
        ui.allocate_rect(
            Rect::from_min_max(used.min, egui::pos2(used.min.x + width, used.max.y)),
            Sense::hover(),
        );
        inner
    }

    fn show_area<R>(
        self,
        ui: &mut Ui,
        key: Id,
        gutter: Option<(f32, f32)>,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let mut memory: Memory = ui.ctx().data(|d| d.get_temp(key)).unwrap_or_default();
        let hovered = memory.rect.is_some_and(|r| ui.rect_contains_pointer(r));
        let offset = ease_wheel(ui, &mut memory, hovered);

        let content_style = ui.style().clone();
        if let Some((gutter, _)) = gutter {
            bar_style(ui, gutter);
        }
        let mut area = ScrollArea::vertical()
            .id_salt(self.id_salt)
            .auto_shrink(self.auto_shrink);
        if let Some(h) = self.max_height {
            area = area.max_height(h);
        }
        if let Some(y) = offset {
            area = area.vertical_scroll_offset(y);
        }
        let out = area.show(ui, |ui| {
            // The content keeps the page's look; only the bar takes the gutter style.
            ui.set_style(content_style);
            if let Some((_, width)) = gutter {
                // The same width whether or not the bar is showing.
                ui.set_max_width(width);
            }
            add_contents(ui)
        });
        memory.rect = Some(out.inner_rect.union(ui.min_rect()));
        memory.offset = out.state.clamped_offset().y;
        memory.max_offset = (out.content_size.y - out.inner_rect.height()).max(0.0);
        ui.ctx().data_mut(|d| d.insert_temp(key, memory));
        out.inner
    }
}

/// Takes a wheel's movement over the area out of egui's input and eases it into the offset.
/// Returns the offset to show this frame, if the wheel moved it.
fn ease_wheel(ui: &Ui, memory: &mut Memory, hovered: bool) -> Option<f32> {
    let (now, dt, wheel) = ui.input(|i| {
        let wheel = i.events.iter().any(|e| match e {
            Event::MouseWheel { unit, delta, .. } => match unit {
                MouseWheelUnit::Line | MouseWheelUnit::Page => true,
                MouseWheelUnit::Point => delta.length() >= WHEEL_POINTS,
            },
            _ => false,
        });
        (i.time, i.stable_dt.clamp(0.0, 0.1), wheel)
    });
    if hovered && wheel {
        memory.wheel_until = now + WHEEL_HOLD;
    }
    if hovered && now < memory.wheel_until {
        let delta = ui.ctx().input_mut(|i| {
            let d = i.smooth_scroll_delta.y;
            i.smooth_scroll_delta.y = 0.0;
            d
        });
        memory.pending += delta;
    }
    if memory.pending == 0.0 {
        return None;
    }
    let step = if memory.pending.abs() <= SETTLE {
        memory.pending
    } else {
        memory.pending * egui::emath::exponential_smooth_factor(EASE_FRACTION, EASE_SECONDS, dt)
    };
    memory.pending -= step;
    // A positive delta moves the content down, which is a smaller offset.
    let wanted = memory.offset - step;
    let offset = wanted.clamp(0.0, memory.max_offset);
    if offset != wanted || memory.pending.abs() <= f32::EPSILON {
        // At an end, or done: nothing more to ease in.
        memory.pending = 0.0;
    } else {
        ui.ctx().request_repaint();
    }
    memory.offset = offset;
    Some(offset)
}

/// A solid bar (it takes room rather than floating over the content), centred in a gutter
/// `gutter` points wide: a muted pill on no track, brighter under the pointer.
fn bar_style(ui: &mut Ui, gutter: f32) {
    let bar = BAR.min(gutter * 0.5);
    let margin = ((gutter - bar) * 0.5).max(0.0);
    let style = ui.style_mut();
    style.spacing.scroll = ScrollStyle {
        bar_width: bar,
        bar_inner_margin: margin,
        bar_outer_margin: margin,
        ..ScrollStyle::solid()
    };
    let visuals = &mut style.visuals;
    let handle = visuals.widgets.noninteractive.fg_stroke.color;
    visuals.extreme_bg_color = Color32::TRANSPARENT;
    visuals.widgets.inactive.bg_fill = handle.gamma_multiply(0.35);
    visuals.widgets.hovered.bg_fill = handle.gamma_multiply(0.55);
    visuals.widgets.active.bg_fill = handle.gamma_multiply(0.7);
    let round = egui::CornerRadius::same((bar * 0.5).round() as u8);
    visuals.widgets.inactive.corner_radius = round;
    visuals.widgets.hovered.corner_radius = round;
    visuals.widgets.active.corner_radius = round;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(ctx: &egui::Context, lines: f32, time: f64, show: &mut dyn FnMut(&mut Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 300.0),
            )),
            time: Some(time),
            events: vec![
                Event::PointerMoved(egui::pos2(100.0, 100.0)),
                Event::MouseWheel {
                    unit: MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, lines),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
            .into_iter()
            .filter(|e| lines != 0.0 || matches!(e, Event::PointerMoved(_)))
            .collect(),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| show(ui));
        // No renderer here: drop the texture uploads on purpose.
        out.textures_delta.clear();
    }

    /// Runs frames 1/60 s apart, wheeling `lines` on frame `at`, and returns the offset after
    /// each frame.
    fn offsets(gutter: Option<f32>, frames: usize, at: usize, lines: f32) -> Vec<f32> {
        let ctx = egui::Context::default();
        let mut seen = Vec::new();
        for f in 0..frames {
            let mut offset = 0.0;
            let mut show = |ui: &mut Ui| {
                let mut s = SmoothScroll::vertical("t").auto_shrink(false);
                if let Some(g) = gutter {
                    s = s.gutter(g);
                }
                s.show(ui, |ui| {
                    for i in 0..100 {
                        ui.label(format!("row {i}"));
                    }
                });
                let id = ui.make_persistent_id(("smooth-scroll", "t"));
                offset = ui
                    .ctx()
                    .data(|d| d.get_temp::<Memory>(id))
                    .map_or(0.0, |m| m.offset);
            };
            let l = if f == at { lines } else { 0.0 };
            wheel(&ctx, -l, f as f64 / 60.0, &mut show);
            seen.push(offset);
        }
        seen
    }

    #[test]
    fn a_wheel_notch_eases_in_over_several_frames() {
        let seen = offsets(None, 40, 3, 3.0);
        let end = *seen.last().unwrap_or(&0.0);
        assert!(end > 50.0, "scrolled down: {seen:?}");
        // Not all at once: a frame after the notch, well short of where it ends.
        assert!(seen[5] > 0.0 && seen[5] < end * 0.6, "eased: {seen:?}");
        // Moving down all the way, never back.
        assert!(
            seen.windows(2).all(|w| w[1] >= w[0] - 0.01),
            "monotonic: {seen:?}"
        );
        // Settled by the end.
        assert!((seen[38] - end).abs() < 0.01, "settled: {seen:?}");
    }

    #[test]
    fn wheeling_up_at_the_top_stays_at_the_top() {
        let seen = offsets(Some(32.0), 20, 3, -3.0);
        assert!(seen.iter().all(|o| *o == 0.0), "{seen:?}");
    }

    #[test]
    fn the_gutter_keeps_the_content_width_and_holds_the_bar() {
        let ctx = egui::Context::default();
        let mut widths = Vec::new();
        for f in 0..3 {
            wheel(&ctx, 0.0, f as f64 / 60.0, &mut |ui: &mut Ui| {
                let page = ui.available_width();
                ui.set_max_width(page - 32.0);
                let want = ui.available_width();
                SmoothScroll::vertical("g").gutter(32.0).show(ui, |ui| {
                    widths.push((want, ui.available_width()));
                    for i in 0..100 {
                        ui.label(format!("row {i}"));
                    }
                });
            });
        }
        for (want, got) in widths {
            assert!((want - got).abs() < 0.5, "content {got} wide, page {want}");
        }
    }

    #[test]
    fn the_bar_is_centred_in_the_gutter() {
        let mut ui_style = egui::Style::default();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            bar_style(ui, 32.0);
            ui_style = (**ui.style()).clone();
        });
        out.textures_delta.clear();
        let s = ui_style.spacing.scroll;
        assert!(!s.floating);
        assert_eq!(s.bar_inner_margin, s.bar_outer_margin);
        assert!((s.allocated_width() - 32.0).abs() < 0.01);
    }
}
