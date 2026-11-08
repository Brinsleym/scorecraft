//! Settings (owner, 2026-11-01): a page of the library, opened from the bottom of its sidebar.
//! Nothing in the score view opens it. Each change is an engine or shell command, so it is
//! saved like the rest of the preferences and an agent can make the same change.

use egui::{RichText, Ui};
use serde_json::{Value, json};

use super::{App, LAYOUTS};
use crate::state::Theme;
use crate::theme::Tokens;

fn group(ui: &mut Ui, tokens: &Tokens, title: &str) {
    ui.add_space(14.0);
    ui.label(RichText::new(title).size(16.0).strong().color(tokens.text));
    ui.add_space(2.0);
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
        egui::Frame::NONE
            .fill(tokens.toolbar_bg)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Settings")
                                .size(22.0)
                                .strong()
                                .color(tokens.text),
                        );

                        group(ui, tokens, "Appearance");
                        ui.horizontal(|ui| {
                            for (theme, label) in [(Theme::Light, "Light"), (Theme::Dark, "Dark")] {
                                if ui.radio(self.state.theme == theme, label).clicked() {
                                    run.push(("ui.theme.set", json!({ "theme": theme.id() })));
                                }
                            }
                        });

                        group(ui, tokens, "Screen");
                        let mut awake = self.state.keep_awake.wanted;
                        if ui
                            .checkbox(&mut awake, "Keep the screen awake while a score is open")
                            .changed()
                        {
                            run.push(("ui.power.keepAwake", json!({ "on": awake })));
                        }
                        if let Some(e) = &self.state.keep_awake.error {
                            ui.label(RichText::new(e).weak());
                        }

                        group(ui, tokens, "Page turning");
                        let mut on = swipe;
                        if ui.checkbox(&mut on, "Swipe to turn pages").changed() {
                            run.push(("input.swipe.set", json!({ "on": on })));
                        }
                        ui.label(
                            RichText::new(
                                "For every score; a score's own layout and turns, chosen in the \
                                 score view, come first.",
                            )
                            .weak(),
                        );
                        let dl = defaults
                            .get("layout")
                            .and_then(Value::as_str)
                            .unwrap_or("auto");
                        let dm = defaults
                            .get("mode")
                            .and_then(Value::as_str)
                            .unwrap_or("full");
                        ui.horizontal(|ui| {
                            ui.label("Page layout:");
                            for (id, label, _) in LAYOUTS {
                                if ui.radio(dl == id, label).clicked() {
                                    run.push(("view.defaults.set", json!({ "layout": id })));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Turns:");
                            for (id, label) in [
                                ("full", "Whole page"),
                                ("halfPage", "Half page"),
                                ("alternating", "Alternating"),
                            ] {
                                if ui.radio(dm == id, label).clicked() {
                                    run.push(("view.defaults.set", json!({ "mode": id })));
                                }
                            }
                        });
                        ui.label(
                            RichText::new("Tap the left half to go back, the right half to go on.")
                                .weak(),
                        );

                        group(ui, tokens, "Markup");
                        let mut ruler = self.settings.show_ruler_button;
                        if ui.checkbox(&mut ruler, "Show the ruler button").changed() {
                            self.settings.show_ruler_button = ruler;
                            self.save();
                        }
                        let mut round = flag("highlighterRoundEnds");
                        if ui.checkbox(&mut round, "Round highlighter ends").changed() {
                            run.push(("annotate.highlighter.roundEnds", json!({ "on": round })));
                        }
                        let mut shapes = flag("shapes");
                        if ui
                            .checkbox(&mut shapes, "Shapes: straighten lines and circles")
                            .changed()
                        {
                            run.push(("annotate.shapes", json!({ "on": shapes })));
                        }
                        let current = markup
                            .get("penButton")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        ui.horizontal(|ui| {
                            ui.label("The pen's button switches to:");
                            for (id, label) in [
                                ("eraser", "Eraser"),
                                ("highlighter", "Highlighter"),
                                ("pen", "Pen"),
                                ("text", "Text"),
                                ("select", "Select"),
                            ] {
                                if ui.radio(current == id, label).clicked() {
                                    run.push(("annotate.pen.buttonTool", json!({ "tool": id })));
                                }
                            }
                        });

                        if !cfg!(target_arch = "wasm32") {
                            group(ui, tokens, "Sync");
                            ui.horizontal(|ui| {
                                crate::library_view::sync_buttons(
                                    ui,
                                    &mut self.session,
                                    &mut actions,
                                );
                            });
                        }
                    });
            });
        for (id, p) in run {
            let r = self.command(id, &p);
            self.report(r);
        }
        self.library_actions(actions);
    }
}
