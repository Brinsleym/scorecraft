//! The shell's icons: Tabler Icons (MIT, outline, stroke 2), drawn from a subset of Tabler's
//! icon font bundled in `assets/tabler/` (`docs/ui-design.md`, Styling rules). Every icon is drawn
//! into a square in one colour; next to text it is [`crate::theme::icon_for`] that text's size.

use egui::{Align2, Color32, FontId, Painter, Rect, Response, Sense, Ui, Vec2, pos2, vec2};

use crate::theme::icon_for;

/// Every icon the shell draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Previous,
    Next,
    Single,
    Two,
    /// The automatic page layout: one page or two, from the window's shape.
    AutoLayout,
    Half,
    Alternate,
    FitWidth,
    ZoomIn,
    ZoomOut,
    FullScreen,
    Theme,
    Markup,
    /// Clear every marking on the pages shown.
    ClearInk,
    Library,
    Lists,
    Recordings,
    Practice,
    Search,
    Trash,
    Settings,
    Folder,
    NewFolder,
    Score,
    Up,
    Down,
    Back,
    Close,
    Play,
    TurnLeft,
    TurnRight,
    Star,
    StarFilled,
    Sync,
    Info,
    More,
    Filter,
    Sort,
    Plus,
    ChevronDown,
    Check,
    /// Move to a folder.
    MoveTo,
    AddToList,
    /// Export or share a copy.
    Share,
    Print,
}

impl Icon {
    /// The icon's glyph in the bundled Tabler subset (`assets/tabler/`), with its Tabler name.
    pub fn glyph(self) -> char {
        match self {
            Icon::Previous => '\u{ea60}',                 // chevron-left
            Icon::Next => '\u{ea61}',                     // chevron-right
            Icon::Single => '\u{eaa4}',                   // file
            Icon::Two => '\u{f6d5}',                      // columns-2
            Icon::AutoLayout => '\u{eadb}',               // layout
            Icon::Half => '\u{ead8}',                     // layout-rows
            Icon::Alternate => '\u{edb0}',                // arrows-left-right
            Icon::FitWidth => '\u{ef37}',                 // arrow-autofit-width
            Icon::ZoomIn => '\u{eb56}',                   // zoom-in
            Icon::ZoomOut => '\u{eb57}',                  // zoom-out
            Icon::FullScreen => '\u{eaea}',               // maximize
            Icon::Theme => '\u{f4a3}',                    // sun-moon
            Icon::Markup => '\u{eb04}',                   // pencil
            Icon::ClearInk => '\u{eb8b}',                 // eraser
            Icon::Library => '\u{eff2}',                  // books
            Icon::Lists => '\u{ec42}',                    // menu-2
            Icon::Recordings => '\u{eaf0}',               // microphone
            Icon::Practice => '\u{ea5c}',                 // chart-line
            Icon::Search => '\u{eb1c}',                   // search
            Icon::Trash => '\u{eb41}',                    // trash
            Icon::Settings => '\u{eb20}',                 // settings
            Icon::Folder => '\u{eaad}',                   // folder
            Icon::NewFolder => '\u{eaab}',                // folder-plus
            Icon::Score => '\u{ea9f}',                    // file-music
            Icon::Up => '\u{ea62}',                       // chevron-up
            Icon::Down | Icon::ChevronDown => '\u{ea5f}', // chevron-down
            Icon::Back => '\u{ea19}',                     // arrow-left
            Icon::Close => '\u{eb55}',                    // x
            Icon::Play => '\u{ed46}',                     // player-play
            Icon::TurnLeft => '\u{eb16}',                 // rotate
            Icon::TurnRight => '\u{eb15}',                // rotate-clockwise
            Icon::Star => '\u{eb2e}',                     // star
            Icon::StarFilled => '\u{f6a6}',               // star-filled
            Icon::Sync => '\u{eb13}',                     // refresh
            Icon::Info => '\u{eac5}',                     // info-circle
            Icon::More => '\u{ea95}',                     // dots
            Icon::Filter => '\u{eaa5}',                   // filter
            Icon::Sort => '\u{eb5a}',                     // arrows-sort
            Icon::Plus => '\u{eb0b}',                     // plus
            Icon::Check => '\u{ea5e}',                    // check
            Icon::MoveTo => '\u{f91b}',                   // folder-symlink
            Icon::AddToList => '\u{f008}',                // playlist-add
            Icon::Share => '\u{eb47}',                    // upload
            Icon::Print => '\u{eb0e}',                    // printer
        }
    }
}

/// Paints `icon` filling the square `rect` (its smaller side is the icon's size) in `c`.
pub fn paint(p: &Painter, rect: Rect, c: Color32, icon: Icon) {
    let side = rect.width().min(rect.height());
    if side > 0.0 {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon.glyph(),
            FontId::new(side, crate::theme::icon_family()),
            c,
        );
    }
}

/// Paints `icon` at `side` points, centred in `rect` (a whole button).
pub fn paint_centered(p: &Painter, rect: Rect, side: f32, c: Color32, icon: Icon) {
    paint(
        p,
        Rect::from_center_size(rect.center(), Vec2::splat(side)),
        c,
        icon,
    );
}

/// A small icon-only button for dialogs and rows, `side` points square, with `hover` as its
/// tooltip.
pub fn small_button(ui: &mut Ui, icon: Icon, hover: &str) -> Response {
    small_button_sized(ui, icon, hover, 24.0, false)
}

/// [`small_button`] at another size, optionally shown selected.
pub fn small_button_sized(
    ui: &mut Ui,
    icon: Icon,
    hover: &str,
    side: f32,
    selected: bool,
) -> Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact_selectable(&r, selected);
        if r.hovered() || selected || r.has_focus() {
            ui.painter()
                .rect_filled(rect, v.corner_radius, v.weak_bg_fill);
        }
        let text = egui::TextStyle::Button.resolve(ui.style()).size;
        paint_centered(
            ui.painter(),
            rect,
            icon_for(text).min(side),
            v.text_color(),
            icon,
        );
    }
    r.on_hover_text(hover)
}

/// A button with an icon before its text, the height of a normal button.
pub fn labelled(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    labelled_selected(ui, icon, text, false)
}

/// [`labelled`], shown selected when `selected`.
pub fn labelled_selected(ui: &mut Ui, icon: Icon, text: &str, selected: bool) -> Response {
    let pad = ui.spacing().button_padding;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let side = icon_for(font.size);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let size = vec2(
        pad.x * 2.0 + side + 6.0 + galley.size().x,
        (pad.y * 2.0 + side).max(ui.spacing().interact_size.y),
    );
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact_selectable(&r, selected);
        ui.painter().rect(
            rect,
            v.corner_radius,
            v.weak_bg_fill,
            v.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let icon_rect = Rect::from_min_size(
            pos2(rect.left() + pad.x, rect.center().y - side / 2.0),
            Vec2::splat(side),
        );
        paint(ui.painter(), icon_rect, v.text_color(), icon);
        ui.painter().galley(
            pos2(
                icon_rect.right() + 6.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            v.text_color(),
        );
    }
    r
}

/// A full-width row with an icon and its text: the library sidebar's rows.
pub fn row(ui: &mut Ui, icon: Icon, text: &str, selected: bool) -> Response {
    let pad = ui.spacing().button_padding;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let side = icon_for(font.size);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let size = vec2(
        ui.available_width().max(side + pad.x * 2.0),
        (pad.y * 2.0 + side + 4.0).max(ui.spacing().interact_size.y),
    );
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact_selectable(&r, selected);
        if selected || r.hovered() {
            ui.painter()
                .rect_filled(rect, v.corner_radius, v.weak_bg_fill);
        }
        let icon_rect = Rect::from_min_size(
            pos2(rect.left() + pad.x + 2.0, rect.center().y - side / 2.0),
            Vec2::splat(side),
        );
        paint(ui.painter(), icon_rect, v.text_color(), icon);
        let clip = Rect::from_min_max(pos2(icon_rect.right() + 8.0, rect.top()), rect.max);
        ui.painter().with_clip_rect(clip).galley(
            pos2(clip.left(), rect.center().y - galley.size().y / 2.0),
            galley,
            v.text_color(),
        );
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon paints something inside its rectangle, at any size, without panicking.
    #[test]
    fn every_icon_paints_inside_its_box() {
        let ctx = egui::Context::default();
        let all = [
            Icon::Previous,
            Icon::Next,
            Icon::Single,
            Icon::Two,
            Icon::AutoLayout,
            Icon::Half,
            Icon::Alternate,
            Icon::FitWidth,
            Icon::ZoomIn,
            Icon::ZoomOut,
            Icon::FullScreen,
            Icon::Theme,
            Icon::Markup,
            Icon::ClearInk,
            Icon::Library,
            Icon::Lists,
            Icon::Recordings,
            Icon::Practice,
            Icon::Search,
            Icon::Trash,
            Icon::Settings,
            Icon::Folder,
            Icon::NewFolder,
            Icon::Score,
            Icon::Up,
            Icon::Down,
            Icon::Back,
            Icon::Close,
            Icon::Play,
            Icon::TurnLeft,
            Icon::TurnRight,
            Icon::Star,
            Icon::StarFilled,
            Icon::Sync,
            Icon::Info,
            Icon::More,
            Icon::Filter,
            Icon::Sort,
            Icon::Plus,
            Icon::ChevronDown,
            Icon::Check,
        ];
        for side in [0.0, 16.0, 44.0] {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let rect = Rect::from_min_size(pos2(100.0, 100.0), Vec2::splat(side));
                for icon in all {
                    paint(ui.painter(), rect, Color32::WHITE, icon);
                }
            });
            out.textures_delta.clear();
            if side > 0.0 {
                assert!(
                    out.shapes.len() >= all.len(),
                    "{side}: {}",
                    out.shapes.len()
                );
            }
        }
    }
}

#[cfg(test)]
mod subset_tests {
    use super::Icon;
    use skrifa::MetadataProvider;

    const ALL: [Icon; 45] = [
        Icon::Previous,
        Icon::Next,
        Icon::Single,
        Icon::Two,
        Icon::AutoLayout,
        Icon::Half,
        Icon::Alternate,
        Icon::FitWidth,
        Icon::ZoomIn,
        Icon::ZoomOut,
        Icon::FullScreen,
        Icon::Theme,
        Icon::Markup,
        Icon::ClearInk,
        Icon::Library,
        Icon::Lists,
        Icon::Recordings,
        Icon::Practice,
        Icon::Search,
        Icon::Trash,
        Icon::Settings,
        Icon::Folder,
        Icon::NewFolder,
        Icon::Score,
        Icon::Up,
        Icon::Down,
        Icon::Back,
        Icon::Close,
        Icon::Play,
        Icon::TurnLeft,
        Icon::TurnRight,
        Icon::Star,
        Icon::StarFilled,
        Icon::Sync,
        Icon::Info,
        Icon::More,
        Icon::Filter,
        Icon::Sort,
        Icon::Plus,
        Icon::ChevronDown,
        Icon::Check,
        Icon::MoveTo,
        Icon::AddToList,
        Icon::Share,
        Icon::Print,
    ];

    /// Every icon's glyph is in the bundled subset: a missing one would draw as a box.
    #[test]
    fn every_icon_is_in_the_subset() {
        let font = skrifa::FontRef::new(include_bytes!("../assets/tabler/tabler-icons.ttf"))
            .expect("the bundled font parses");
        let map = font.charmap();
        for icon in ALL {
            assert!(
                map.map(icon.glyph()).is_some(),
                "{icon:?} is not in the subset"
            );
        }
    }
}
