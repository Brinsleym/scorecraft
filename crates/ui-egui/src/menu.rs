//! Menu rows (`docs/ui-design.md`, Styling rules: Menus): every context menu and dropdown in the
//! shell is built from these, inside egui's menu popups (whose panel [`crate::theme::Tokens`]
//! styles). A row is 32 points high: a 16-point Tabler icon, the 13-point label, and on the right a
//! check (the choice in force) or a chevron (opens a submenu).

use egui::containers::menu::{MenuState, SubMenu};
use egui::{Color32, Rect, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

/// A row's height, side padding, and the gap between icon and label.
const ROW: f32 = 32.0;
const PAD: f32 = 12.0;
const GAP: f32 = 12.0;
/// The narrowest a menu is drawn.
const MIN_WIDTH: f32 = 200.0;

/// What a row does, which decides what it shows on its right.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Plain,
    /// The choice in force when `true`: a check on the right.
    Choice(bool),
    /// Opens a submenu: a chevron on the right.
    Sub,
    /// Destroys something: drawn in the danger colour.
    Danger,
}

/// A menu row; `None` for the icon leaves its space blank so labels line up.
pub fn item(ui: &mut Ui, icon: Option<Icon>, text: &str) -> Response {
    row(ui, icon, text, Kind::Plain)
}

/// A row for one of several choices, checked while it is the one in force.
pub fn choice(ui: &mut Ui, icon: Option<Icon>, text: &str, on: bool) -> Response {
    row(ui, icon, text, Kind::Choice(on))
}

/// A destructive row (Delete): its text and icon in the danger colour.
pub fn danger(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    row(ui, Some(icon), text, Kind::Danger)
}

/// A row that opens `content` as a submenu beside it, on hover, click or the arrow key.
pub fn submenu<R>(
    ui: &mut Ui,
    icon: Option<Icon>,
    text: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let r = row(ui, icon, text, Kind::Sub);
    SubMenu::new().show(ui, &r, content).map(|i| i.inner)
}

/// A line between groups of rows.
pub fn separator(ui: &mut Ui) {
    let t = Tokens::current(ui.ctx());
    // As wide as the menu's widest row (the menu's layout is justified); asking for the
    // available width would stretch the menu during egui's sizing pass.
    let (rect, _) = ui.allocate_at_least(vec2(0.0, 9.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, t.divider),
    );
}

/// A note or group title in a menu ("No lists yet", "New blank score"): small and muted.
pub fn note(ui: &mut Ui, text: &str) {
    let t = Tokens::current(ui.ctx());
    ui.spacing_mut().item_spacing.y = 0.0;
    let font = egui::FontId::proportional(theme::CAPTION);
    let galley = ui
        .painter()
        .layout(text.to_owned(), font, t.muted, MIN_WIDTH - PAD * 2.0);
    let size = vec2(galley.size().x + PAD * 2.0, galley.size().y + 12.0);
    let (rect, _) = ui.allocate_at_least(size, Sense::hover());
    ui.painter()
        .galley(pos2(rect.left() + PAD, rect.top() + 6.0), galley, t.muted);
}

fn row(ui: &mut Ui, icon: Option<Icon>, text: &str, kind: Kind) -> Response {
    let t = Tokens::current(ui.ctx());
    // Rows sit edge to edge, as in the concept's menus.
    ui.spacing_mut().item_spacing.y = 0.0;
    let font = egui::FontId::proportional(theme::TEXT);
    let icon_px = theme::icon_for(theme::TEXT);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let trailing = match kind {
        Kind::Choice(_) | Kind::Sub => GAP + icon_px,
        _ => 0.0,
    };
    let want = PAD + icon_px + GAP + galley.size().x + trailing + PAD;
    let (rect, r) = ui.allocate_at_least(vec2(want.max(MIN_WIDTH), ROW), Sense::click());
    let enabled = ui.is_enabled();
    r.widget_info(|| match kind {
        Kind::Choice(on) => WidgetInfo::selected(Role::RadioButton, enabled, on, text),
        _ => WidgetInfo::labeled(Role::Button, enabled, text),
    });
    if !ui.is_rect_visible(rect) {
        return r;
    }
    let open = kind == Kind::Sub
        && MenuState::from_ui(ui, |s, _| {
            s.open_item == Some(SubMenu::id_from_widget_id(r.id))
        });
    let hot = enabled && (r.hovered() || r.has_focus() || open);
    let (mut fg, mut glyph) = match kind {
        Kind::Danger => (t.danger, t.danger),
        _ if hot => (t.strong, t.strong),
        _ => (t.text, t.nav_text),
    };
    if !enabled {
        fg = t.muted;
        glyph = t.muted;
    }
    let p = ui.painter();
    if hot {
        let fill = if kind == Kind::Danger {
            t.danger_hover
        } else {
            t.button_hover
        };
        p.rect_filled(rect, 4, fill);
    }
    let y = rect.center().y;
    let square = |x: f32, side: f32| Rect::from_min_size(pos2(x, y - side / 2.0), vec2(side, side));
    let mut x = rect.left() + PAD;
    if let Some(icon) = icon {
        icons::paint(p, square(x, icon_px), glyph, icon);
    }
    x += icon_px + GAP;
    p.galley(pos2(x, y - galley.size().y / 2.0), galley, fg);
    let right = square(rect.right() - PAD - icon_px, icon_px);
    match kind {
        Kind::Choice(true) => icons::paint(p, right, t.accent, Icon::Check),
        Kind::Sub => icons::paint_centered(p, right, 12.0, t.muted, Icon::Next),
        _ => {}
    }
    r
}
