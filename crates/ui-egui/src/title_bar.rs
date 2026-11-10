//! The window's own title bar (owner's mockup, 2026-11-08), drawn by ScoreCraft on desktops whose
//! window has no system frame ([`crate::Platform::draws_title_bar`]).
//!
//! Left to right: Back, the sidebar toggle, the app icon and "ScoreCraft"; on the right
//! Minimise, Maximise (Restore when maximised) and Close, Windows-style. The empty middle drags
//! the window (the system's own move, so Windows snap and Aero Shake still work) and a
//! double-click maximises or restores it. [`resize_edges`] gives the frameless window its resize
//! borders back, and [`paint_border`] its hairline edge. In a window (not maximised) the bar's
//! top corners are rounded ([`corners`]); the desktop app makes the window transparent so the
//! corners show what is behind it.
//!
//! Each button answers with a [`TitleAction`]; the app runs the matching `ui.*` command, so agents
//! drive the same actions over the control channel.

use egui::{
    Align2, Color32, Context, CornerRadius, CursorIcon, FontId, Id, LayerId, Order, Pos2, Rect,
    ResizeDirection, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::icons::{self, Icon};
use crate::theme::{self, BRAND, Tokens};

/// The bar's height, in points: the sidebar rows' and header buttons' 36 (the mockup's).
pub const HEIGHT: f32 = 36.0;
/// Back and the sidebar toggle: their width, and the gap before the first.
const NAV_BUTTON: f32 = 40.0;
const LEFT_PAD: f32 = 4.0;
/// The app icon's side, and the gaps either side of it.
const LOGO: f32 = 16.0;
const LOGO_GAP: f32 = 12.0;
const NAME_GAP: f32 = 8.0;
/// Minimise, Maximise and Close: Windows' 46-point-wide caption buttons, and their 10-point glyphs.
const WINDOW_BUTTON: f32 = 46.0;
const GLYPH: f32 = 10.0;
/// The invisible resize borders: their depth, and how far a corner reaches along each edge.
const RESIZE_EDGE: f32 = 5.0;
const RESIZE_CORNER: f32 = 14.0;
/// The radius of the window's top corners while it is not maximised: Windows 11's 8.
const CORNER: u8 = 8;

/// The window's corner radii: the top two rounded while it is not maximised, square otherwise.
pub fn corners(maximized: bool) -> CornerRadius {
    if maximized {
        CornerRadius::ZERO
    } else {
        CornerRadius {
            nw: CORNER,
            ne: CORNER,
            sw: 0,
            se: 0,
        }
    }
}

/// What a press in the title bar asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleAction {
    Back,
    Sidebar,
    Minimize,
    /// Maximise, or restore when maximised (the button, or a double-click on the bar).
    Maximize,
    Close,
}

impl TitleAction {
    /// The shell command behind the action, with its params.
    pub fn command(self) -> &'static str {
        match self {
            TitleAction::Back => "ui.back",
            TitleAction::Sidebar => "ui.sidebar.toggle",
            TitleAction::Minimize => "ui.window.minimize",
            TitleAction::Maximize => "ui.window.maximize",
            TitleAction::Close => "ui.window.close",
        }
    }
}

/// What the bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TitleBar {
    /// Back has somewhere to go (otherwise it is greyed out).
    pub can_go_back: bool,
    /// The sidebar is shown (the toggle is drawn selected).
    pub sidebar: bool,
    /// The window is maximised: Maximise shows Restore.
    pub maximized: bool,
}

/// Draws the bar filling `ui` (the title-bar panel) and returns the button pressed, if any.
pub fn show(ui: &mut Ui, tokens: &Tokens, bar: TitleBar) -> Option<TitleAction> {
    let rect = ui.max_rect();
    let mut out = None;

    // The middle drags the window. Registered first, so the buttons sit on top of it.
    let right = rect.right() - 3.0 * WINDOW_BUTTON;
    let nav_end = rect.left() + LEFT_PAD + 2.0 * NAV_BUTTON;
    if right > nav_end {
        let drag = Rect::from_min_max(pos2(nav_end, rect.top()), pos2(right, rect.bottom()));
        let r = ui.interact(
            drag,
            ui.scope_id().with("title-drag"),
            Sense::click_and_drag(),
        );
        if r.double_clicked() {
            out = Some(TitleAction::Maximize);
        } else if r.drag_started_by(egui::PointerButton::Primary) {
            // The system's own move: Windows snap, Aero Shake and Win+arrow keep working.
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }

    // Back and the sidebar toggle.
    let mut x = rect.left() + LEFT_PAD;
    let nav = |x: f32| Rect::from_min_size(pos2(x, rect.top()), vec2(NAV_BUTTON, rect.height()));
    if nav_button(ui, tokens, nav(x), Icon::Back, bar.can_go_back, false)
        .on_hover_text("Back")
        .clicked()
    {
        out = Some(TitleAction::Back);
    }
    x += NAV_BUTTON;
    if nav_button(ui, tokens, nav(x), Icon::Menu, true, false)
        .on_hover_text(if bar.sidebar {
            "Hide the sidebar"
        } else {
            "Show the sidebar"
        })
        .clicked()
    {
        out = Some(TitleAction::Sidebar);
    }
    x += NAV_BUTTON + LOGO_GAP;

    // The app icon and name.
    let painter = ui
        .painter()
        .with_clip_rect(rect.with_max_x(right.max(rect.left())));
    let logo = Rect::from_min_size(pos2(x, rect.center().y - LOGO / 2.0), Vec2::splat(LOGO));
    paint_logo(&painter, logo);
    painter.text(
        pos2(logo.right() + NAME_GAP, rect.center().y),
        Align2::LEFT_CENTER,
        "ScoreCraft",
        FontId::proportional(theme::TEXT + 1.0),
        tokens.text,
    );

    // Minimise, Maximise or Restore, Close.
    let win = |i: f32| {
        Rect::from_min_size(
            pos2(rect.right() - (3.0 - i) * WINDOW_BUTTON, rect.top()),
            vec2(WINDOW_BUTTON, rect.height()),
        )
    };
    let buttons = [
        (Glyph::Minimize, "Minimise", TitleAction::Minimize),
        (
            if bar.maximized {
                Glyph::Restore
            } else {
                Glyph::Maximize
            },
            if bar.maximized { "Restore" } else { "Maximise" },
            TitleAction::Maximize,
        ),
        (Glyph::Close, "Close", TitleAction::Close),
    ];
    for (i, (glyph, label, action)) in buttons.into_iter().enumerate() {
        let corner = if glyph == Glyph::Close {
            CornerRadius {
                ne: corners(bar.maximized).ne,
                ..CornerRadius::ZERO
            }
        } else {
            CornerRadius::ZERO
        };
        if window_button(ui, tokens, win(i as f32), glyph, corner)
            .on_hover_text(label)
            .clicked()
        {
            out = Some(action);
        }
    }

    // The hairline under the bar.
    ui.painter()
        .hline(rect.x_range(), rect.bottom() - 0.5, (1.0, tokens.divider));
    out
}

/// Back or the sidebar toggle: a rounded hover fill and a 16-point Tabler icon.
fn nav_button(
    ui: &mut Ui,
    tokens: &Tokens,
    rect: Rect,
    icon: Icon,
    enabled: bool,
    selected: bool,
) -> egui::Response {
    let id = ui.scope_id().with(("title-nav", icon.glyph()));
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let r = ui.interact(rect, id, sense);
    let fill_rect = Rect::from_center_size(rect.center(), vec2(rect.width() - 8.0, 28.0));
    if enabled && (r.hovered() || selected) {
        ui.painter().rect_filled(
            fill_rect,
            CornerRadius::same(tokens.radius / 2),
            tokens.button_hover,
        );
    }
    let colour = if enabled { tokens.icon } else { tokens.muted };
    icons::paint_centered(
        ui.painter(),
        rect,
        theme::icon_for(theme::TEXT),
        colour,
        icon,
    );
    r
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Glyph {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// A Windows-style caption button: square, a hover fill (red for Close), and a thin glyph.
/// `corner` rounds its hover fill (Close's top-right, in a window).
fn window_button(
    ui: &mut Ui,
    tokens: &Tokens,
    rect: Rect,
    glyph: Glyph,
    corner: CornerRadius,
) -> egui::Response {
    let r = ui.interact(
        rect,
        ui.scope_id().with(("title-window", glyph as u8)),
        Sense::click(),
    );
    let close = glyph == Glyph::Close;
    let mut colour = tokens.icon;
    if r.hovered() || r.is_pointer_button_down_on() {
        let fill = if close {
            colour = Color32::WHITE;
            tokens.close_hover
        } else {
            tokens.button_hover
        };
        ui.painter().rect_filled(rect, corner, fill);
    }
    paint_glyph(ui.painter(), rect.center(), glyph, colour);
    r
}

fn paint_glyph(p: &egui::Painter, c: Pos2, glyph: Glyph, colour: Color32) {
    let stroke = Stroke::new(1.0, colour);
    let h = GLYPH / 2.0;
    // Snap to half points so the 1-point lines stay crisp at 100 %.
    let c = pos2(c.x.round() + 0.5, c.y.round() + 0.5);
    match glyph {
        Glyph::Minimize => {
            p.hline((c.x - h)..=(c.x + h), c.y, stroke);
        }
        Glyph::Maximize => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(GLYPH)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Middle,
            );
        }
        Glyph::Restore => {
            // A front window, and the corner of one behind it.
            let o = 2.0;
            let front = Rect::from_min_size(pos2(c.x - h, c.y - h + o), Vec2::splat(GLYPH - o));
            p.rect_stroke(front, CornerRadius::same(1), stroke, StrokeKind::Middle);
            p.line(
                vec![
                    pos2(c.x - h + o, c.y - h + o),
                    pos2(c.x - h + o, c.y - h),
                    pos2(c.x + h, c.y - h),
                    pos2(c.x + h, c.y + h - o),
                    pos2(c.x + h - o, c.y + h - o),
                ],
                stroke,
            );
        }
        Glyph::Close => {
            p.line_segment([pos2(c.x - h, c.y - h), pos2(c.x + h, c.y + h)], stroke);
            p.line_segment([pos2(c.x - h, c.y + h), pos2(c.x + h, c.y - h)], stroke);
        }
    }
}

/// The app icon (`assets/app-icon/scorecraft.svg`) painted in `rect`: the white eighth note on
/// the brand's rounded square, from the SVG's own 512-unit geometry so it is sharp at any scale.
pub fn paint_logo(p: &egui::Painter, rect: Rect) {
    let s = rect.width().min(rect.height()) / 512.0;
    if s <= 0.0 {
        return;
    }
    let at = |x: f32, y: f32| rect.min + vec2(x, y) * s;
    let radius = |r: f32| CornerRadius::same((r * s).round().clamp(0.0, 255.0) as u8);
    p.rect_filled(
        Rect::from_min_size(rect.min, Vec2::splat(512.0 * s)),
        radius(112.0),
        BRAND,
    );
    let white = Color32::WHITE;
    // Head, stem and flag.
    p.circle_filled(at(228.0, 312.0), 52.0 * s, white);
    p.rect_filled(
        Rect::from_min_max(at(254.0, 160.0), at(280.0, 312.0)),
        CornerRadius::ZERO,
        white,
    );
    p.rect_filled(
        Rect::from_min_max(at(254.0, 148.0), at(336.0, 188.0)),
        radius(8.0),
        white,
    );
}

/// Which way a press at `pos` resizes a window of `size` points, if it is on a border.
pub fn resize_direction(pos: Pos2, size: Vec2) -> Option<ResizeDirection> {
    let (w, h) = (size.x, size.y);
    if !(pos.x.is_finite() && pos.y.is_finite()) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (l, r) = (pos.x <= RESIZE_EDGE, pos.x >= w - RESIZE_EDGE);
    let (t, b) = (pos.y <= RESIZE_EDGE, pos.y >= h - RESIZE_EDGE);
    if !(l || r || t || b) {
        return None;
    }
    // On a border: near a corner, it resizes both ways.
    let (cl, cr) = (pos.x <= RESIZE_CORNER, pos.x >= w - RESIZE_CORNER);
    let (ct, cb) = (pos.y <= RESIZE_CORNER, pos.y >= h - RESIZE_CORNER);
    use ResizeDirection as D;
    Some(match () {
        _ if ct && cl => D::NorthWest,
        _ if ct && cr => D::NorthEast,
        _ if cb && cl => D::SouthWest,
        _ if cb && cr => D::SouthEast,
        _ if t => D::North,
        _ if b => D::South,
        _ if l => D::West,
        _ => D::East,
    })
}

fn cursor(d: ResizeDirection) -> CursorIcon {
    use ResizeDirection as D;
    match d {
        D::North => CursorIcon::ResizeNorth,
        D::South => CursorIcon::ResizeSouth,
        D::East => CursorIcon::ResizeEast,
        D::West => CursorIcon::ResizeWest,
        D::NorthEast => CursorIcon::ResizeNorthEast,
        D::NorthWest => CursorIcon::ResizeNorthWest,
        D::SouthEast => CursorIcon::ResizeSouthEast,
        D::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

/// Invisible resize borders round a frameless window: thin strips on the foreground layer, so
/// they win over whatever is drawn under them. A press starts the system's own resize.
pub fn resize_edges(ctx: &Context) {
    let screen = ctx.content_rect();
    let (w, h) = (screen.width(), screen.height());
    if w <= 2.0 * RESIZE_CORNER || h <= 2.0 * RESIZE_CORNER {
        return;
    }
    let o = screen.min;
    let strips = [
        Rect::from_min_size(o, vec2(w, RESIZE_EDGE)),
        Rect::from_min_size(o + vec2(0.0, h - RESIZE_EDGE), vec2(w, RESIZE_EDGE)),
        Rect::from_min_size(
            o + vec2(0.0, RESIZE_EDGE),
            vec2(RESIZE_EDGE, h - 2.0 * RESIZE_EDGE),
        ),
        Rect::from_min_size(
            o + vec2(w - RESIZE_EDGE, RESIZE_EDGE),
            vec2(RESIZE_EDGE, h - 2.0 * RESIZE_EDGE),
        ),
    ];
    for (i, strip) in strips.into_iter().enumerate() {
        egui::Area::new(Id::unique(("scorecraft-resize-edge", i)))
            .order(Order::Foreground)
            .fixed_pos(strip.min)
            .constrain(false)
            .sense(Sense::hover())
            .show(ctx, |ui| {
                let (_, r) = ui.allocate_exact_size(strip.size(), Sense::drag());
                let Some(pos) = r.hover_pos().or_else(|| r.interact_pointer_pos()) else {
                    return;
                };
                let Some(dir) = resize_direction(pos - o.to_vec2(), screen.size()) else {
                    return;
                };
                ui.ctx().set_cursor_icon(cursor(dir));
                if r.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                }
            });
    }
}

/// The frameless window's hairline edge, drawn over everything (not when maximised), following
/// the rounded top corners.
pub fn paint_border(ctx: &Context, tokens: &Tokens) {
    let p = ctx.layer_painter(LayerId::new(
        Order::Foreground,
        Id::unique("scorecraft-window-edge"),
    ));
    p.rect_stroke(
        ctx.content_rect(),
        corners(false),
        Stroke::new(1.0, tokens.window_border),
        StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borders_and_corners_resize_the_right_way() {
        use ResizeDirection as D;
        let size = vec2(800.0, 600.0);
        let at = |x, y| resize_direction(pos2(x, y), size);
        assert_eq!(at(400.0, 300.0), None);
        assert_eq!(at(400.0, 1.0), Some(D::North));
        assert_eq!(at(400.0, 599.0), Some(D::South));
        assert_eq!(at(1.0, 300.0), Some(D::West));
        assert_eq!(at(799.0, 300.0), Some(D::East));
        assert_eq!(at(1.0, 1.0), Some(D::NorthWest));
        assert_eq!(
            at(10.0, 2.0),
            Some(D::NorthWest),
            "a corner reaches along the edge"
        );
        assert_eq!(at(2.0, 10.0), Some(D::NorthWest));
        assert_eq!(at(799.0, 1.0), Some(D::NorthEast));
        assert_eq!(at(1.0, 599.0), Some(D::SouthWest));
        assert_eq!(at(795.0, 597.0), Some(D::SouthEast));
        assert_eq!(at(f32::NAN, 1.0), None);
        assert_eq!(resize_direction(pos2(0.0, 0.0), Vec2::ZERO), None);
    }

    #[test]
    fn only_a_window_has_rounded_top_corners() {
        let w = corners(false);
        assert_eq!((w.nw, w.ne, w.sw, w.se), (CORNER, CORNER, 0, 0));
        assert_eq!(corners(true), CornerRadius::ZERO);
    }

    #[test]
    fn actions_name_shell_commands() {
        for a in [
            TitleAction::Back,
            TitleAction::Sidebar,
            TitleAction::Minimize,
            TitleAction::Maximize,
            TitleAction::Close,
        ] {
            assert!(
                crate::SHELL_COMMANDS.iter().any(|c| c.0 == a.command()),
                "{a:?}"
            );
        }
    }

    #[test]
    fn the_bar_draws_and_reports_nothing_untouched() {
        let ctx = Context::default();
        let tokens = Tokens::for_theme(crate::Theme::Dark);
        let mut got = Some(TitleAction::Close);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            got = show(
                ui,
                &tokens,
                TitleBar {
                    can_go_back: false,
                    sidebar: true,
                    maximized: true,
                },
            );
            resize_edges(ui.ctx());
            paint_border(ui.ctx(), &tokens);
        });
        out.textures_delta.clear();
        assert_eq!(got, None);
        assert!(!out.shapes.is_empty());
    }
}
