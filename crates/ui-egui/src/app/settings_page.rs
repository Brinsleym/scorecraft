//! Settings (owner, 2026-11-01): a page of the library, opened from the bottom of its sidebar.
//! Nothing in the score view opens it. Each change is an engine or shell command, so it is
//! saved like the rest of the preferences and an agent can make the same change.
//!
//! Laid out from the owner's Stitch concept (*ScoreCraft - Settings* and its *Alt*, 2026-11-08):
//! titled sections of cards on a dark canvas, switches, segmented controls and theme previews.
//! The concept's parts with nothing behind them yet (MIDI and pedal pairing, Wi-Fi pairing, the
//! cache meter) are left out until they exist.

use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};
use serde_json::{Value, json};

use super::{App, LAYOUTS};
use crate::icons::{self, Icon};
use crate::library_view::LibraryAction;
use crate::state::Theme;
use crate::theme::{self, BRAND, BRAND_LIGHT, Tokens};

/// The concept's spacing, in points: the page's least side margin (owner, 2026-11-08: wider than
/// the concept's 40 and a narrower column, centred), top and bottom margin, widest column, between
/// sections, and inside a card.
const MARGIN_X: f32 = 96.0;
const MARGIN_Y: f32 = 32.0;
const MAX_WIDTH: f32 = 760.0;
const SECTION_GAP: f32 = 28.0;
const CARD_PAD: i8 = 16;
/// The 14-point section titles and row labels (the concept's `text-sm`).
const LABEL: f32 = 14.0;

/// Which theme card is chosen.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ThemeChoice {
    Light,
    Dark,
    System,
}

impl App {
    pub(super) fn settings_page(&mut self, ui: &mut Ui, tokens: &Tokens) {
        let mut run: Vec<(&'static str, Value)> = Vec::new();
        let mut actions = Vec::new();
        let markup = self
            .session
            .dispatch("annotate.state", &json!({}))
            .unwrap_or(Value::Null);
        let flag = |k: &str| markup.get(k).and_then(Value::as_bool).unwrap_or(false);
        let swipe = self
            .session
            .dispatch("input.swipe.set", &json!({}))
            .ok()
            .and_then(|v| v.get("on").and_then(Value::as_bool))
            .unwrap_or(true);
        let defaults = self
            .session
            .dispatch("view.defaults.set", &json!({}))
            .ok()
            .and_then(|v| v.get("defaults").cloned())
            .unwrap_or(Value::Null);
        let sync = (!cfg!(target_arch = "wasm32")).then(|| {
            self.session
                .dispatch("sync.status", &json!({}))
                .unwrap_or(Value::Null)
        });
        let library = self
            .session
            .dispatch("library.info", &json!({}))
            .unwrap_or(Value::Null);
        let now = self.session.now_ms();
        let chosen = if self.state.theme_system {
            ThemeChoice::System
        } else if self.state.theme == Theme::Dark {
            ThemeChoice::Dark
        } else {
            ThemeChoice::Light
        };

        egui::Frame::NONE.fill(tokens.page_bg).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            // The scroll bar sits over the right margin, clear of the centred column.
            crate::scroll::SmoothScroll::vertical("settings")
                .auto_shrink(false)
                .show(ui, |ui| {
                    // The column is centred: equal margins, at least MARGIN_X.
                    let side = ((ui.available_width() - MAX_WIDTH) / 2.0).max(MARGIN_X);
                    let width = (ui.available_width() - side * 2.0).max(0.0);
                    ui.add_space(MARGIN_Y);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_space(side);
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            title(ui, tokens);

                            section(ui, tokens, Icon::Palette, "Appearance");
                            let cards = [
                                (ThemeChoice::Light, Icon::Sun, "Light", "light"),
                                (ThemeChoice::Dark, Icon::Moon, "Dark", "dark"),
                                (ThemeChoice::System, Icon::SystemTheme, "System", "system"),
                            ];
                            let gap = 12.0;
                            let w = ((ui.available_width() - gap * 2.0) / 3.0).max(80.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = gap;
                                for (choice, icon, label, id) in cards {
                                    let r = theme_card(
                                        ui,
                                        tokens,
                                        w,
                                        choice,
                                        icon,
                                        label,
                                        chosen == choice,
                                    );
                                    if r.clicked() && chosen != choice {
                                        run.push(("ui.theme.set", json!({ "theme": id })));
                                    }
                                }
                            });
                            divider(ui, tokens);

                            section(ui, tokens, Icon::Screen, "Screen");
                            card(ui, tokens, |ui| {
                                let awake = self.state.keep_awake.wanted;
                                if switch_row(
                                    ui,
                                    tokens,
                                    Icon::Sun,
                                    "Keep screen awake during score view",
                                    awake,
                                    false,
                                )
                                .clicked()
                                {
                                    run.push(("ui.power.keepAwake", json!({ "on": !awake })));
                                }
                                if let Some(e) = &self.state.keep_awake.error {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(e).size(theme::SMALL).color(tokens.error),
                                    );
                                }
                            });
                            divider(ui, tokens);

                            section(ui, tokens, Icon::Book, "Page Turning");
                            card(ui, tokens, |ui| {
                                if switch_row(
                                    ui,
                                    tokens,
                                    Icon::Swipe,
                                    "Swipe to turn pages",
                                    swipe,
                                    false,
                                )
                                .clicked()
                                {
                                    run.push(("input.swipe.set", json!({ "on": !swipe })));
                                }
                                row_rule(ui, tokens, 14.0, 16.0);
                                let defaults_hint = "For every score; a score's own layout and \
                                                     turns, chosen in the score view, come first";
                                let dl = defaults
                                    .get("layout")
                                    .and_then(Value::as_str)
                                    .unwrap_or("auto");
                                let labels: Vec<&str> = LAYOUTS.iter().map(|l| l.1).collect();
                                let at = LAYOUTS.iter().position(|l| l.0 == dl);
                                if let Some(i) = segmented_row(
                                    ui,
                                    tokens,
                                    Icon::PageLayout,
                                    "Page layout",
                                    &labels,
                                    at,
                                    defaults_hint,
                                ) && let Some(l) = LAYOUTS.get(i)
                                {
                                    run.push(("view.defaults.set", json!({ "layout": l.0 })));
                                }
                                row_rule(ui, tokens, 14.0, 16.0);
                                let turns = [
                                    ("full", "Whole page"),
                                    ("halfPage", "Half page"),
                                    ("alternating", "Alternating"),
                                ];
                                let dm = defaults
                                    .get("mode")
                                    .and_then(Value::as_str)
                                    .unwrap_or("full");
                                let labels: Vec<&str> = turns.iter().map(|t| t.1).collect();
                                let at = turns.iter().position(|t| t.0 == dm);
                                if let Some(i) = segmented_row(
                                    ui,
                                    tokens,
                                    Icon::Turns,
                                    "Turns",
                                    &labels,
                                    at,
                                    "Tap the left half to go back, the right half to go on",
                                ) && let Some(t) = turns.get(i)
                                {
                                    run.push(("view.defaults.set", json!({ "mode": t.0 })));
                                }
                            });
                            divider(ui, tokens);

                            section(ui, tokens, Icon::Markup, "Markup");
                            card(ui, tokens, |ui| {
                                let ruler = self.settings.show_ruler_button;
                                if switch_row(
                                    ui,
                                    tokens,
                                    Icon::Ruler,
                                    "Show ruler button",
                                    ruler,
                                    true,
                                )
                                .clicked()
                                {
                                    self.settings.show_ruler_button = !ruler;
                                    self.save();
                                }
                                row_rule(ui, tokens, 12.0, 12.0);
                                let round = flag("highlighterRoundEnds");
                                if switch_row(
                                    ui,
                                    tokens,
                                    Icon::Circle,
                                    "Round highlighter ends",
                                    round,
                                    true,
                                )
                                .clicked()
                                {
                                    run.push((
                                        "annotate.highlighter.roundEnds",
                                        json!({ "on": !round }),
                                    ));
                                }
                                row_rule(ui, tokens, 12.0, 12.0);
                                let shapes = flag("shapes");
                                if switch_row(
                                    ui,
                                    tokens,
                                    Icon::Square,
                                    "Straighten lines and shapes",
                                    shapes,
                                    true,
                                )
                                .clicked()
                                {
                                    run.push(("annotate.shapes", json!({ "on": !shapes })));
                                }
                                row_rule(ui, tokens, 12.0, 20.0);
                                let tools = [
                                    ("eraser", "Eraser"),
                                    ("highlighter", "Highlighter"),
                                    ("pen", "Pen"),
                                    ("text", "Text"),
                                    ("select", "Select"),
                                ];
                                let current = markup.get("penButton").and_then(Value::as_str);
                                caption(ui, tokens, Icon::Bolt, "Pen button action");
                                ui.add_space(8.0);
                                let labels: Vec<&str> = tools.iter().map(|t| t.1).collect();
                                let at = tools.iter().position(|t| Some(t.0) == current);
                                if let Some(i) = segmented_grid(ui, tokens, &labels, at)
                                    && let Some(t) = tools.get(i)
                                {
                                    run.push(("annotate.pen.buttonTool", json!({ "tool": t.0 })));
                                }
                            });

                            if let Some(status) = &sync {
                                divider(ui, tokens);
                                section(ui, tokens, Icon::CloudUpload, "Sync");
                                card(ui, tokens, |ui| {
                                    sync_card(ui, tokens, status, now, &mut actions);
                                });
                            }

                            divider(ui, tokens);
                            section(ui, tokens, Icon::Database, "Storage");
                            card(ui, tokens, |ui| storage_card(ui, tokens, &library));

                            divider(ui, tokens);
                            ui.add_space(16.0);
                            footer(ui, tokens);
                            ui.add_space(64.0);
                        });
                    });
                });
        });
        for (id, p) in run {
            let r = self.command(id, &p);
            self.report(r);
        }
        self.library_actions(actions);
    }
}

fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

fn semibold(size: f32) -> FontId {
    FontId::new(size, theme::semibold())
}

/// A rounded square with an icon in it: section badges and row icons.
fn badge(
    ui: &mut Ui,
    side: f32,
    radius: u8,
    fill: Color32,
    edge: Color32,
    icon: (Icon, f32, Color32),
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(radius),
        fill,
        Stroke::new(1.0, edge),
        StrokeKind::Inside,
    );
    icons::paint_centered(p, rect, icon.1, icon.2, icon.0);
}

/// The page title: the settings icon in a brand-tinted badge, *Settings*, and a rule.
fn title(ui: &mut Ui, c: &Tokens) {
    ui.horizontal(|ui| {
        badge(
            ui,
            32.0,
            8,
            BRAND.gamma_multiply(0.1),
            BRAND.gamma_multiply(0.2),
            (Icon::Settings, 20.0, BRAND),
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new("Settings")
                .font(semibold(theme::SELECTION_TITLE))
                .extra_letter_spacing(theme::SELECTION_TITLE * theme::TIGHT)
                .color(c.strong),
        );
    });
    ui.add_space(16.0);
    hline(ui, c.rule);
}

/// A section's heading: its icon badge and its title in capitals.
fn section(ui: &mut Ui, c: &Tokens, icon: Icon, title: &str) {
    ui.add_space(SECTION_GAP);
    ui.horizontal(|ui| {
        badge(ui, 28.0, 8, c.badge, c.badge_edge, (icon, 16.0, BRAND));
        ui.add_space(10.0);
        ui.label(
            RichText::new(title.to_uppercase())
                .font(semibold(LABEL))
                .extra_letter_spacing(LABEL * 0.05)
                .color(c.text),
        );
    });
    ui.add_space(12.0);
}

/// The line between two sections.
fn divider(ui: &mut Ui, c: &Tokens) {
    ui.add_space(SECTION_GAP);
    hline(ui, c.section_rule);
}

fn hline(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, color));
}

/// The line between two rows of a card, `above` and `below` points from them.
fn row_rule(ui: &mut Ui, c: &Tokens, above: f32, below: f32) {
    ui.add_space(above);
    hline(ui, c.rule);
    ui.add_space(below);
}

/// A card: rounded, edged, padded, the column's full width.
fn card<R>(ui: &mut Ui, c: &Tokens, body: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(c.panel)
        .stroke(Stroke::new(1.0, c.panel_edge))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(CARD_PAD))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui)
        })
        .inner
}

/// The small icon well at the start of a row: 32 points (28 in a compact row).
fn row_icon(ui: &mut Ui, c: &Tokens, icon: Icon, compact: bool) {
    let (side, radius, glyph) = if compact {
        (28.0, 4, 14.0)
    } else {
        (32.0, 8, 16.0)
    };
    badge(
        ui,
        side,
        radius,
        c.inset,
        c.panel_edge,
        (icon, glyph, c.muted),
    );
}

/// A switch, `on` or off, eased between the two.
fn paint_switch(ui: &Ui, c: &Tokens, rect: Rect, on: bool, id: egui::Id) {
    let t = ui.ctx().animate_bool_responsive(id, on);
    let p = ui.painter();
    let track = lerp_color(c.switch_off, BRAND, t);
    p.rect_filled(rect, CornerRadius::same(12), track);
    let knob = lerp_color(c.knob_off, Color32::WHITE, t);
    let r = (rect.height() - 4.0) / 2.0;
    let x = egui::lerp(rect.left() + 2.0 + r..=rect.right() - 2.0 - r, t);
    p.circle_filled(
        pos2(x, rect.center().y + 0.5),
        r,
        Color32::from_black_alpha(40),
    );
    p.circle_filled(pos2(x, rect.center().y), r, knob);
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// A row with an icon, a label and a switch at its end; the whole row toggles it.
fn switch_row(
    ui: &mut Ui,
    c: &Tokens,
    icon: Icon,
    label: &str,
    on: bool,
    compact: bool,
) -> Response {
    let height = if compact { 28.0 } else { 32.0 };
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, true, on, label));
    if ui.is_rect_visible(rect) {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.spacing_mut().item_spacing = vec2(0.0, 0.0);
        row_icon(&mut child, c, icon, compact);
        child.add_space(12.0);
        let color = if compact && !r.hovered() {
            c.text
        } else {
            c.strong
        };
        child.label(RichText::new(label).font(font(LABEL)).color(color));
        let track = Rect::from_min_size(
            pos2(rect.right() - 44.0, rect.center().y - 12.0),
            vec2(44.0, 24.0),
        );
        paint_switch(ui, c, track, on, r.id);
        if r.has_focus() {
            ui.painter().rect_stroke(
                track.expand(2.0),
                CornerRadius::same(14),
                Stroke::new(2.0, BRAND),
                StrokeKind::Outside,
            );
        }
    }
    r
}

/// The size of a segmented control's button for `label`.
fn segment_size(ui: &Ui, label: &str) -> Vec2 {
    let g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font(theme::SMALL), Color32::PLACEHOLDER);
    vec2(g.size().x + 24.0, 28.0)
}

/// Paints one segment of a segmented control and returns whether it was clicked.
fn segment(ui: &mut Ui, c: &Tokens, rect: Rect, label: &str, selected: bool, id: egui::Id) -> bool {
    let r = ui.interact(rect, id, Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, selected, label));
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(6), BRAND);
    } else if r.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), c.strong.gamma_multiply(0.05));
    }
    let color = if selected {
        c.on_accent
    } else if r.hovered() {
        c.strong
    } else {
        c.muted
    };
    let f = if selected {
        semibold(theme::SMALL)
    } else {
        font(theme::SMALL)
    };
    p.text(rect.center(), Align2::CENTER_CENTER, label, f, color);
    if r.has_focus() {
        p.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(2.0, BRAND),
            StrokeKind::Inside,
        );
    }
    r.clicked() && !selected
}

/// The well a segmented control sits in.
fn paint_well(ui: &Ui, c: &Tokens, rect: Rect) {
    ui.painter().rect(
        rect,
        CornerRadius::same(8),
        c.inset,
        Stroke::new(1.0, c.panel_edge),
        StrokeKind::Inside,
    );
}

/// A row with an icon and label, and a segmented control at its end. Returns the segment
/// clicked.
fn segmented_row(
    ui: &mut Ui,
    c: &Tokens,
    icon: Icon,
    label: &str,
    options: &[&str],
    selected: Option<usize>,
    hint: &str,
) -> Option<usize> {
    let sizes: Vec<Vec2> = options.iter().map(|o| segment_size(ui, o)).collect();
    let control = vec2(sizes.iter().map(|s| s.x).sum::<f32>() + 8.0, 38.0);
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), control.y), Sense::hover());
    let id = r.id;
    r.on_hover_text(hint);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing = vec2(0.0, 0.0);
    row_icon(&mut child, c, icon, false);
    child.add_space(12.0);
    child.label(RichText::new(label).font(font(LABEL)).color(c.strong));
    let well = Rect::from_min_size(pos2(rect.right() - control.x, rect.top()), control);
    paint_well(ui, c, well);
    let mut x = well.left() + 4.0;
    let mut clicked = None;
    for (i, (o, s)) in options.iter().zip(&sizes).enumerate() {
        let seg = Rect::from_min_size(pos2(x, well.top() + 5.0), *s);
        if segment(ui, c, seg, o, selected == Some(i), id.with(i)) {
            clicked = Some(i);
        }
        x += s.x;
    }
    clicked
}

/// A segmented control filling the row, its segments the same width.
fn segmented_grid(
    ui: &mut Ui,
    c: &Tokens,
    options: &[&str],
    selected: Option<usize>,
) -> Option<usize> {
    let (well, r) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::hover());
    paint_well(ui, c, well);
    let n = options.len().max(1) as f32;
    let gap = 6.0;
    let w = (well.width() - 8.0 - gap * (n - 1.0)) / n;
    let mut clicked = None;
    for (i, o) in options.iter().enumerate() {
        let x = well.left() + 4.0 + (w + gap) * i as f32;
        let seg = Rect::from_min_size(pos2(x, well.top() + 4.0), vec2(w, 34.0));
        if segment(ui, c, seg, o, selected == Some(i), r.id.with(i)) {
            clicked = Some(i);
        }
    }
    clicked
}

/// A small capitalised caption with a brand icon: *Pen button action*.
fn caption(ui: &mut Ui, c: &Tokens, icon: Icon, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        icons::paint(ui.painter(), rect, BRAND, icon);
        ui.label(
            RichText::new(text.to_uppercase())
                .font(semibold(theme::SMALL))
                .extra_letter_spacing(theme::SMALL * 0.05)
                .color(c.muted),
        );
    });
}

/// One of the three theme cards: a small picture of the theme, its icon and name, and a radio
/// mark. The chosen card has a brand edge and an *Active* tag.
fn theme_card(
    ui: &mut Ui,
    c: &Tokens,
    width: f32,
    choice: ThemeChoice,
    icon: Icon,
    label: &str,
    selected: bool,
) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(width, 114.0), Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, selected, label));
    if !ui.is_rect_visible(rect) {
        return r;
    }
    let p = ui.painter();
    let edge = if selected {
        Stroke::new(2.0, BRAND)
    } else if r.hovered() {
        Stroke::new(1.0, c.muted)
    } else {
        Stroke::new(1.0, c.panel_edge)
    };
    p.rect(
        rect,
        CornerRadius::same(12),
        c.panel,
        edge,
        StrokeKind::Inside,
    );
    if r.has_focus() {
        p.rect_stroke(
            rect.expand(2.0),
            CornerRadius::same(14),
            Stroke::new(2.0, BRAND),
            StrokeKind::Outside,
        );
    }
    let inner = rect.shrink(12.0);
    let preview = Rect::from_min_size(inner.min, vec2(inner.width(), 64.0));
    paint_preview(ui, preview, choice);

    let y = preview.bottom() + 10.0 + 12.0;
    let icon_rect = Rect::from_center_size(pos2(inner.left() + 8.0, y), Vec2::splat(16.0));
    let icon_color = if selected { BRAND } else { c.muted };
    icons::paint(p, icon_rect, icon_color, icon);
    let (f, color) = if selected {
        (semibold(LABEL), c.strong)
    } else if r.hovered() {
        (font(LABEL), c.strong)
    } else {
        (font(LABEL), c.text)
    };
    let text = p.text(
        pos2(icon_rect.right() + 8.0, y),
        Align2::LEFT_CENTER,
        label,
        f,
        color,
    );
    if selected {
        let g = p.layout_no_wrap("Active".into(), font(theme::CAPTION), BRAND_LIGHT);
        let tag = Rect::from_min_size(
            pos2(text.right() + 8.0, y - g.size().y / 2.0 - 2.0),
            g.size() + vec2(12.0, 4.0),
        );
        p.rect_filled(tag, CornerRadius::same(4), BRAND.gamma_multiply(0.2));
        p.galley(tag.min + vec2(6.0, 2.0), g, BRAND_LIGHT);
    }
    let mark = pos2(inner.right() - 8.0, y);
    if selected {
        p.circle_filled(mark, 8.0, BRAND);
        icons::paint_centered(
            p,
            Rect::from_center_size(mark, Vec2::splat(16.0)),
            10.0,
            Color32::WHITE,
            Icon::Check,
        );
    } else {
        p.circle(mark, 7.5, c.inset, Stroke::new(1.0, c.badge_edge));
    }
    r
}

/// A theme card's picture: a page of that theme, or day beside night for *System*. These keep
/// their colours in either app theme, since they show the themes themselves.
fn paint_preview(ui: &Ui, rect: Rect, choice: ThemeChoice) {
    let p = ui.painter().with_clip_rect(rect);
    let day = Palette {
        bg: Color32::from_rgb(0xe8, 0xe4, 0xdc),
        edge: Color32::from_rgb(0xd0, 0xcc, 0xc4),
        dot: Color32::from_rgb(0xbb, 0xbb, 0xbb),
        text: Color32::from_rgb(0x66, 0x66, 0x66),
        bar: [
            Color32::from_rgb(0x88, 0x88, 0x88),
            Color32::from_rgb(0xaa, 0xaa, 0xaa),
        ],
        accent: BRAND.gamma_multiply(0.6),
    };
    let night = Palette {
        bg: Color32::from_rgb(0x15, 0x13, 0x10),
        edge: Color32::from_rgb(0x37, 0x34, 0x31),
        dot: Color32::from_rgb(0x37, 0x34, 0x31),
        text: Color32::from_rgb(0x6b, 0x72, 0x80),
        bar: [
            Color32::from_rgb(0x4b, 0x55, 0x63),
            Color32::from_rgb(0x37, 0x41, 0x51),
        ],
        accent: BRAND,
    };
    let radius = CornerRadius::same(8);
    let tiny = |s: f32| FontId::new(s, theme::semibold());
    let bar = |r: Rect, y: f32, frac: f32, color: Color32| {
        let b = Rect::from_min_size(
            pos2(r.left() + 8.0, y),
            vec2((r.width() - 16.0) * frac, 4.0),
        );
        p.rect_filled(b, CornerRadius::same(2), color);
    };
    match choice {
        ThemeChoice::Light | ThemeChoice::Dark => {
            let c = if choice == ThemeChoice::Light {
                &day
            } else {
                &night
            };
            p.rect(
                rect,
                radius,
                c.bg,
                Stroke::new(1.0, c.edge),
                StrokeKind::Inside,
            );
            for i in 0..2 {
                p.circle_filled(
                    pos2(rect.left() + 11.0 + i as f32 * 10.0, rect.top() + 12.0),
                    3.0,
                    c.dot,
                );
            }
            p.text(
                pos2(rect.right() - 8.0, rect.top() + 12.0),
                Align2::RIGHT_CENTER,
                "SCORE",
                tiny(9.0),
                c.text,
            );
            bar(rect, rect.top() + 26.0, 0.8, c.bar[0]);
            bar(rect, rect.top() + 34.0, 0.6, c.bar[1]);
            p.rect_filled(
                Rect::from_min_size(
                    pos2(rect.left() + 8.0, rect.bottom() - 14.0),
                    vec2(24.0, 6.0),
                ),
                CornerRadius::same(2),
                c.accent,
            );
        }
        ThemeChoice::System => {
            p.rect_filled(rect, radius, night.bg);
            let (l, rt) = rect.split_left_right_at_fraction(0.5);
            p.rect_filled(
                l,
                CornerRadius {
                    nw: 8,
                    sw: 8,
                    ne: 0,
                    se: 0,
                },
                day.bg,
            );
            for (half, c, word) in [(l, &day, "DAY"), (rt, &night, "NIGHT")] {
                p.text(
                    pos2(half.left() + 8.0, half.top() + 12.0),
                    Align2::LEFT_CENTER,
                    word,
                    tiny(8.0),
                    c.text,
                );
                bar(half, half.bottom() - 14.0, 0.75, c.bar[0]);
            }
            p.vline(
                l.right(),
                rect.y_range(),
                Stroke::new(1.0, Color32::from_rgb(0x33, 0x33, 0x33)),
            );
            p.rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, night.edge),
                StrokeKind::Inside,
            );
        }
    }
}

struct Palette {
    bg: Color32,
    edge: Color32,
    dot: Color32,
    text: Color32,
    bar: [Color32; 2],
    accent: Color32,
}

/// A secondary button (*Sync Now*): an icon and text on the button colour.
fn button(ui: &mut Ui, c: &Tokens, icon: Icon, icon_color: Color32, text: &str) -> Response {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font(theme::SMALL), Color32::PLACEHOLDER);
    let size = vec2(16.0 + 14.0 + 6.0 + g.size().x + 16.0, 30.0);
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let fill = if r.hovered() {
        c.button_hover
    } else {
        c.button_bg
    };
    p.rect(
        rect,
        CornerRadius::same(8),
        fill,
        Stroke::new(1.0, c.button_border),
        StrokeKind::Inside,
    );
    if r.has_focus() {
        p.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(2.0, BRAND),
            StrokeKind::Inside,
        );
    }
    let icon_rect = Rect::from_min_size(
        pos2(rect.left() + 16.0, rect.center().y - 7.0),
        Vec2::splat(14.0),
    );
    icons::paint(p, icon_rect, icon_color, icon);
    p.galley(
        pos2(icon_rect.right() + 6.0, rect.center().y - g.size().y / 2.0),
        g,
        c.strong,
    );
    r
}

/// A primary button (*Connect Folder*): white on the brand colour.
fn primary_button(ui: &mut Ui, c: &Tokens, icon: Icon, text: &str) -> Response {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font(theme::SMALL), Color32::PLACEHOLDER);
    let size = vec2(12.0 + 14.0 + 6.0 + g.size().x + 12.0, 30.0);
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let fill = if r.hovered() { c.accent_hover } else { BRAND };
    p.rect_filled(rect, CornerRadius::same(8), fill);
    if r.has_focus() {
        p.rect_stroke(
            rect.expand(2.0),
            CornerRadius::same(10),
            Stroke::new(2.0, BRAND),
            StrokeKind::Outside,
        );
    }
    let icon_rect = Rect::from_min_size(
        pos2(rect.left() + 12.0, rect.center().y - 7.0),
        Vec2::splat(14.0),
    );
    icons::paint(p, icon_rect, c.on_accent, icon);
    p.galley(
        pos2(icon_rect.right() + 6.0, rect.center().y - g.size().y / 2.0),
        g,
        c.on_accent,
    );
    r
}

/// How long ago `then` was, from `now` (Unix milliseconds): *just now*, *5 min ago*, a date.
fn ago(then: u64, now: u64) -> String {
    let mins = now.saturating_sub(then) / 60_000;
    match mins {
        0 => "just now".into(),
        1..=59 => format!("{mins} min ago"),
        60..=1439 => format!("{} h ago", mins / 60),
        _ => crate::library_view::day(then),
    }
}

/// Sync through a shared folder (the concept's *Connected* card, or its *Not Connected* one).
fn sync_card(ui: &mut Ui, c: &Tokens, status: &Value, now: u64, out: &mut Vec<LibraryAction>) {
    let folder = status.get("backend").and_then(Value::as_str);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().item_spacing = vec2(0.0, 0.0);
    match folder {
        Some(folder) => {
            badge(
                &mut row,
                36.0,
                8,
                c.success_bg,
                c.success_edge,
                (Icon::Check, 20.0, c.success),
            );
            row.add_space(12.0);
            row.label(RichText::new("Connected").font(font(LABEL)).color(c.strong));
            let last = status
                .get("lastSyncMs")
                .and_then(Value::as_u64)
                .map_or_else(
                    || "Not synced yet".to_owned(),
                    |t| format!("Last synced {}", ago(t, now)),
                );
            row.add_space(8.0);
            row.label(
                RichText::new(format!("• {last}"))
                    .font(font(theme::SMALL))
                    .color(c.muted),
            )
            .on_hover_text(format!("Syncing through {folder}"));
            row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let mut tip = format!("Syncing through {folder}");
                let waiting = status
                    .get("waitingFiles")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if waiting > 0 {
                    tip.push_str(&format!(
                        "\n{waiting} score files are still on their way from other devices"
                    ));
                }
                if let Some(p) = status
                    .get("problems")
                    .and_then(Value::as_array)
                    .and_then(|a| a.last())
                    .and_then(Value::as_str)
                {
                    tip.push_str(&format!("\nLast problem: {p}"));
                }
                if button(ui, c, Icon::Sync, BRAND, "Sync Now")
                    .on_hover_text(tip)
                    .clicked()
                {
                    out.push(LibraryAction::SyncNow);
                }
                if button(ui, c, Icon::Folder, c.muted, "Change Folder")
                    .on_hover_text("Sync through another folder")
                    .clicked()
                {
                    out.push(LibraryAction::ChooseSyncFolder);
                }
            });
        }
        None => {
            badge(
                &mut row,
                36.0,
                8,
                c.inset,
                c.panel_edge,
                (Icon::Cloud, 20.0, BRAND),
            );
            row.add_space(12.0);
            row.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(
                        RichText::new("Sync folder")
                            .font(font(LABEL))
                            .color(c.strong),
                    );
                    let g = ui.painter().layout_no_wrap(
                        "Not Connected".into(),
                        font(theme::CAPTION),
                        c.muted,
                    );
                    let (tag, _) =
                        ui.allocate_exact_size(g.size() + vec2(12.0, 4.0), Sense::hover());
                    ui.painter().rect(
                        tag,
                        CornerRadius::same(4),
                        c.badge,
                        Stroke::new(1.0, c.badge_edge),
                        StrokeKind::Inside,
                    );
                    ui.painter().galley(tag.min + vec2(6.0, 2.0), g, c.muted);
                });
                ui.label(
                    RichText::new("No folder chosen • Sync is off")
                        .font(FontId::new(theme::SMALL, FontFamily::Monospace))
                        .color(c.muted),
                );
            });
            row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, c, Icon::Plus, "Connect Folder")
                    .on_hover_text(
                        "Sync this library with your other devices through a folder they all \
                         reach: one Google Drive, OneDrive, Dropbox or iCloud Drive keeps in sync",
                    )
                    .clicked()
                {
                    out.push(LibraryAction::ChooseSyncFolder);
                }
            });
        }
    }
}

/// Where the library is kept, and how many scores it holds.
fn storage_card(ui: &mut Ui, c: &Tokens, library: &Value) {
    let scores = library.get("scores").and_then(Value::as_u64).unwrap_or(0);
    let location = library
        .get("location")
        .and_then(Value::as_str)
        .unwrap_or("memory");
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Library")
                .font(font(theme::SMALL))
                .color(c.strong),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let n = if scores == 1 {
                "1 score".to_owned()
            } else {
                format!("{scores} scores")
            };
            ui.label(
                RichText::new(n)
                    .font(FontId::new(theme::SMALL, FontFamily::Monospace))
                    .color(c.muted),
            );
        });
    });
    row_rule(ui, c, 12.0, 12.0);
    let place = match location {
        "memory" => "In memory for this session: markings made now won't be kept",
        "browser" => "In this browser's storage",
        path => path,
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        icons::paint(ui.painter(), rect, c.muted, Icon::Folder);
        ui.add(
            egui::Label::new(
                RichText::new(place)
                    .font(FontId::new(theme::SMALL, FontFamily::Monospace))
                    .color(c.muted),
            )
            .truncate(),
        )
        .on_hover_text(place);
    });
}

/// The version line at the bottom of the page.
fn footer(ui: &mut Ui, c: &Tokens) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 3.0, c.success.gamma_multiply(0.8));
        ui.label(
            RichText::new(format!("ScoreCraft v{}", env!("CARGO_PKG_VERSION")))
                .font(FontId::new(theme::SMALL, FontFamily::Monospace))
                .color(c.muted),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::ago;

    #[test]
    fn sync_times_read_as_ago() {
        let now = 1_793_404_800_000;
        assert_eq!(ago(now, now), "just now");
        assert_eq!(ago(now - 5 * 60_000, now), "5 min ago");
        assert_eq!(ago(now - 3 * 3_600_000, now), "3 h ago");
        assert_eq!(ago(now - 2 * 86_400_000, now), "2026-10-29");
        // A clock that went back doesn't underflow.
        assert_eq!(ago(now + 60_000, now), "just now");
    }
}
