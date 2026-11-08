//! Design tokens (`docs/ui-design.md`): every colour and size the shell draws with.

use craft_ui::buttons::{ButtonVisuals, IconButtonStyle};
use craft_ui::theme::WidgetPalette;
use egui::{Color32, CornerRadius, Stroke};

use crate::state::Theme;

/// ScoreCraft's brand colour (owner, 2026-11-08), the accent in every theme.
pub const BRAND: Color32 = Color32::from_rgb(0xc4, 0x58, 0x21);
/// [`BRAND`] under the pointer.
pub const BRAND_HOVER: Color32 = Color32::from_rgb(0xd2, 0x66, 0x2e);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    /// Behind the pages.
    pub reader_bg: Color32,
    /// Drawn where a page's bitmap isn't ready yet.
    pub placeholder: Color32,
    pub toolbar_bg: Color32,
    pub text: Color32,
    pub icon: Color32,
    pub accent: Color32,
    pub hover: Color32,
    pub error: Color32,
    /// The library window (`docs/ui-design.md`, Library layout): its sidebar, the page beside
    /// it, folder cards, and the header's buttons.
    pub sidebar_bg: Color32,
    pub main_bg: Color32,
    /// Lines between the sidebar and the page, and above Settings.
    pub divider: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub card_border: Color32,
    pub button_bg: Color32,
    pub button_hover: Color32,
    pub button_border: Color32,
    /// Headings, and a selected sidebar row's text.
    pub strong: Color32,
    /// Secondary text: item counts, composers.
    pub muted: Color32,
    /// Sidebar rows: their text, and their fill when hovered and when selected.
    pub nav_text: Color32,
    pub nav_hover: Color32,
    pub nav_selected: Color32,
    /// The accent under the pointer, and text drawn on the accent.
    pub accent_hover: Color32,
    pub on_accent: Color32,
    /// Toolbar height and touch-target side, in points (≥ 44 for fingers on a tablet).
    pub toolbar_height: f32,
    pub button: f32,
    pub radius: u8,
}

impl Tokens {
    pub fn for_theme(theme: Theme) -> Tokens {
        match theme {
            Theme::Light => Tokens {
                reader_bg: Color32::from_rgb(0x5f, 0x63, 0x68),
                placeholder: Color32::from_rgb(0xf4, 0xf4, 0xf2),
                toolbar_bg: Color32::from_rgb(0xf7, 0xf7, 0xf8),
                text: Color32::from_rgb(0x1f, 0x23, 0x28),
                icon: Color32::from_rgb(0x2b, 0x30, 0x36),
                accent: BRAND,
                hover: Color32::from_rgb(0xe3, 0xe6, 0xea),
                error: Color32::from_rgb(0xc0, 0x2b, 0x2b),
                sidebar_bg: Color32::from_rgb(0xf2, 0xf2, 0xf2),
                main_bg: Color32::from_rgb(0xfa, 0xfa, 0xfa),
                divider: Color32::from_rgb(0xdc, 0xdc, 0xdc),
                card: Color32::from_rgb(0xec, 0xec, 0xec),
                card_hover: Color32::from_rgb(0xe2, 0xe2, 0xe2),
                card_border: Color32::from_black_alpha(14),
                button_bg: Color32::WHITE,
                button_hover: Color32::from_rgb(0xf0, 0xf0, 0xf0),
                button_border: Color32::from_black_alpha(26),
                strong: Color32::from_rgb(0x11, 0x11, 0x11),
                muted: Color32::from_rgb(0x6e, 0x6e, 0x6e),
                nav_text: Color32::from_rgb(0x4a, 0x4a, 0x4a),
                nav_hover: Color32::from_rgb(0xe8, 0xe8, 0xe8),
                nav_selected: Color32::from_rgb(0xe2, 0xe2, 0xe2),
                accent_hover: BRAND_HOVER,
                on_accent: Color32::WHITE,
                toolbar_height: 52.0,
                button: 44.0,
                radius: 8,
            },
            Theme::Dark => Tokens {
                reader_bg: Color32::from_rgb(0x12, 0x13, 0x15),
                placeholder: Color32::from_rgb(0x2a, 0x2c, 0x30),
                toolbar_bg: Color32::from_rgb(0x1e, 0x20, 0x23),
                text: Color32::from_rgb(0xe8, 0xea, 0xed),
                icon: Color32::from_rgb(0xdc, 0xdf, 0xe3),
                accent: BRAND,
                hover: Color32::from_rgb(0x34, 0x37, 0x3c),
                error: Color32::from_rgb(0xff, 0x7b, 0x72),
                sidebar_bg: Color32::from_rgb(0x1e, 0x1e, 0x1e),
                main_bg: Color32::from_rgb(0x24, 0x24, 0x24),
                divider: Color32::from_rgb(0x2d, 0x2d, 0x2d),
                card: Color32::from_rgb(0x2c, 0x2c, 0x2c),
                card_hover: Color32::from_rgb(0x33, 0x33, 0x33),
                card_border: Color32::from_white_alpha(13),
                button_bg: Color32::from_rgb(0x33, 0x33, 0x33),
                button_hover: Color32::from_rgb(0x3c, 0x3c, 0x3c),
                button_border: Color32::from_white_alpha(26),
                strong: Color32::WHITE,
                muted: Color32::from_rgb(0x88, 0x88, 0x88),
                nav_text: Color32::from_rgb(0xb0, 0xb0, 0xb0),
                nav_hover: Color32::from_rgb(0x28, 0x28, 0x28),
                nav_selected: Color32::from_rgb(0x2d, 0x2d, 0x2d),
                accent_hover: BRAND_HOVER,
                on_accent: Color32::WHITE,
                toolbar_height: 52.0,
                button: 44.0,
                radius: 8,
            },
        }
    }

    pub fn icon_button(&self) -> IconButtonStyle {
        let v = |fill, icon| ButtonVisuals {
            fill,
            stroke: Stroke::NONE,
            icon,
        };
        IconButtonStyle {
            normal: v(Color32::TRANSPARENT, self.icon),
            hovered: v(self.hover, self.icon),
            pressed: Some(v(self.hover, self.accent)),
            selected: v(self.hover, self.accent),
            corner_radius: CornerRadius::same(self.radius),
            focus_stroke: Stroke::new(2.0, self.accent),
        }
    }

    /// Applies the tokens to egui's visuals.
    pub fn apply(&self, theme: Theme, ctx: &egui::Context) {
        let mut visuals = match theme {
            Theme::Light => egui::Visuals::light(),
            Theme::Dark => egui::Visuals::dark(),
        };
        visuals.panel_fill = self.toolbar_bg;
        visuals.override_text_color = Some(self.text);
        WidgetPalette {
            noninteractive: self.toolbar_bg,
            inactive: self.toolbar_bg,
            hovered: self.hover,
            active: self.hover,
            open: self.hover,
        }
        .apply_to(
            &mut visuals.widgets,
            CornerRadius::same(self.radius),
            Stroke::new(1.0, self.text),
        );
        ctx.set_visuals(visuals);
        ctx.all_styles_mut(|style| {
            use egui::{FontFamily, FontId, TextStyle};
            style.text_styles = [
                (TextStyle::Small, FontId::proportional(CAPTION)),
                (TextStyle::Body, FontId::proportional(TEXT)),
                (TextStyle::Button, FontId::proportional(TEXT)),
                (TextStyle::Heading, FontId::new(TITLE, semibold())),
                (
                    TextStyle::Monospace,
                    FontId::new(TEXT, FontFamily::Monospace),
                ),
            ]
            .into();
        });
    }
}

/// The type scale (owner's Stitch concept, 2026-11-08; `docs/ui-design.md`, Styling rules), in
/// points: body and buttons, card titles, counts and hints, page titles.
pub const TEXT: f32 = 13.0;
pub const SMALL: f32 = 12.0;
pub const CAPTION: f32 = 11.0;
pub const TITLE: f32 = 30.0;
/// Icons in the reader's 44-point toolbar buttons.
pub const TOOLBAR_ICON: f32 = 20.0;
/// "Tight" letter spacing for titles and card captions, as a fraction of the text size.
pub const TIGHT: f32 = -0.025;

/// An icon's size beside text of `text` points: 16 beside 13, as in the concept.
pub fn icon_for(text: f32) -> f32 {
    (text * 16.0 / 13.0).round()
}

/// Segoe UI Semibold (600): page titles. Medium (500) has no Segoe UI face and renders Regular,
/// as it does in a browser.
pub fn semibold() -> egui::FontFamily {
    egui::FontFamily::Name("semibold".into())
}

/// The Tabler icon font alone, so an icon never falls back to a text glyph.
pub fn icon_family() -> egui::FontFamily {
    egui::FontFamily::Name("icons".into())
}

/// Tabler Icons 3.31.0 (MIT), outline at stroke 2, cut to the glyphs [`crate::icons::Icon`] uses.
const TABLER: &[u8] = include_bytes!("../assets/tabler/tabler-icons.ttf");

/// The interface fonts a platform found: Segoe UI's Regular and Semibold faces where the system
/// has them (Windows). ScoreCraft may not ship Segoe UI, so elsewhere egui's own fonts stand in.
#[derive(Debug, Clone, Default)]
pub struct UiFonts {
    pub regular: Option<Vec<u8>>,
    pub semibold: Option<Vec<u8>>,
}

/// Installs the interface fonts: Segoe UI first (when the platform gave valid files), egui's
/// fonts behind it for other scripts, and the Tabler icons. Font bytes from the system are
/// checked first, since egui panics on a file it can't parse.
pub fn install_fonts(ctx: &egui::Context, fonts: &UiFonts) {
    use egui::{FontData, FontFamily};
    use std::sync::Arc;
    let mut defs = egui::FontDefinitions::default();
    let valid = |b: &&Vec<u8>| skrifa::FontRef::from_index(b.as_slice(), 0).is_ok();
    let mut regular = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(bytes) = fonts.regular.as_ref().filter(valid) {
        defs.font_data.insert(
            "segoe-ui".into(),
            Arc::new(FontData::from_owned(bytes.clone())),
        );
        regular.insert(0, "segoe-ui".into());
    }
    let mut bold = regular.clone();
    if let Some(bytes) = fonts.semibold.as_ref().filter(valid) {
        defs.font_data.insert(
            "segoe-ui-semibold".into(),
            Arc::new(FontData::from_owned(bytes.clone())),
        );
        bold.insert(0, "segoe-ui-semibold".into());
    }
    defs.font_data
        .insert("tabler".into(), Arc::new(FontData::from_static(TABLER)));
    defs.families.insert(FontFamily::Proportional, regular);
    defs.families.insert(semibold(), bold);
    defs.families.insert(icon_family(), vec!["tabler".into()]);
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod font_tests {
    use super::*;

    /// A damaged or truncated system font is skipped, not handed to egui (which panics on it);
    /// text and icons still draw.
    #[test]
    fn unreadable_system_fonts_are_skipped() {
        let ctx = egui::Context::default();
        let junk = UiFonts {
            regular: Some(b"not a font".to_vec()),
            semibold: Some(Vec::new()),
        };
        install_fonts(&ctx, &junk);
        Tokens::for_theme(Theme::Dark).apply(Theme::Dark, &ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.heading("Library");
            ui.label("Etude 7");
            crate::icons::paint(
                ui.painter(),
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(16.0, 16.0)),
                Color32::WHITE,
                crate::icons::Icon::Library,
            );
        });
        out.textures_delta.clear();
        assert!(!out.shapes.is_empty());
    }
}
