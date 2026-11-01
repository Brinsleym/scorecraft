//! The Score panel: the open library score's details (LIB-4), its pages (PAGE-1..4) and its
//! bookmarks (LIB-11). Everything it changes goes through engine commands; it keeps only what
//! is being typed.

use egui::{Context, RichText};
use serde_json::{Value, json};

use crate::theme::Tokens;

/// What is typed into the details fields, for one score.
#[derive(Debug, Clone, Default, PartialEq)]
struct Details {
    score: String,
    title: String,
    composer: String,
    arranger: String,
    key: String,
    genre: String,
    tags: String,
}

#[derive(Default)]
pub struct ScorePanel {
    pub open: bool,
    details: Details,
    bookmark: String,
}

fn s(v: &Value, k: &str) -> String {
    v.pointer(k)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

impl ScorePanel {
    pub fn new() -> ScorePanel {
        ScorePanel::default()
    }

    /// The panel as a window. `score` is `library.score.get` for the open score; `reader_page`
    /// is the score page on screen (a position in its pages, 1-based). Returns commands to run.
    pub fn show(
        &mut self,
        ctx: &Context,
        tokens: &Tokens,
        score: &Value,
        reader_page: Option<u64>,
    ) -> Vec<(&'static str, Value)> {
        let mut run = Vec::new();
        let Some(id) = score.get("id").and_then(Value::as_str).map(str::to_owned) else {
            return run;
        };
        if self.details.score != id {
            self.details = Details {
                score: id.clone(),
                title: s(score, "/metadata/title"),
                composer: s(score, "/metadata/composer"),
                arranger: s(score, "/metadata/arranger"),
                key: s(score, "/metadata/key"),
                genre: s(score, "/metadata/genre"),
                tags: score
                    .pointer("/metadata/tags")
                    .and_then(Value::as_array)
                    .map(|t| {
                        t.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default(),
            };
        }
        let mut open = self.open;
        egui::Window::new("Score")
            .open(&mut open)
            .default_width(380.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(RichText::new("Details").strong().color(tokens.text));
                egui::Grid::new("details").num_columns(2).show(ui, |ui| {
                    let d = &mut self.details;
                    for (label, field) in [
                        ("Title", &mut d.title),
                        ("Composer", &mut d.composer),
                        ("Arranger", &mut d.arranger),
                        ("Key", &mut d.key),
                        ("Genre", &mut d.genre),
                        ("Tags", &mut d.tags),
                    ] {
                        ui.label(label);
                        ui.text_edit_singleline(field);
                        ui.end_row();
                    }
                });
                if ui.button("Save details").clicked() {
                    let d = &self.details;
                    let tags: Vec<String> = d
                        .tags
                        .split(',')
                        .map(|t| t.trim().to_owned())
                        .filter(|t| !t.is_empty())
                        .collect();
                    run.push((
                        "score.metadata.set",
                        json!({
                            "score": id, "title": d.title, "composer": d.composer,
                            "arranger": d.arranger, "key": d.key, "genre": d.genre, "tags": tags,
                        }),
                    ));
                }
                ui.separator();
                ui.label(RichText::new("Pages").strong().color(tokens.text));
                let pages = score
                    .get("pages")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let n = pages.len();
                egui::ScrollArea::vertical()
                    .id_salt("pages")
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (i, p) in pages.iter().enumerate() {
                            let pos = i + 1;
                            let source = p.get("source").and_then(Value::as_u64).unwrap_or(0) + 1;
                            let hidden = p.get("hidden").and_then(Value::as_bool).unwrap_or(false);
                            let turn = match p.get("rotation").and_then(Value::as_i64) {
                                Some(90) => " ↻90°",
                                Some(180) => " ↻180°",
                                Some(270) => " ↺90°",
                                _ => "",
                            };
                            ui.horizontal(|ui| {
                                let mut text = RichText::new(format!(
                                    "{pos}. page {source} of the file{turn}"
                                ));
                                if hidden {
                                    text = text.weak().italics();
                                }
                                ui.label(text);
                                if ui.small_button("↺").on_hover_text("Turn left").clicked() {
                                    run.push((
                                        "page.rotate",
                                        json!({ "score": id, "page": pos, "degrees": -90 }),
                                    ));
                                }
                                if ui.small_button("↻").on_hover_text("Turn right").clicked() {
                                    run.push((
                                        "page.rotate",
                                        json!({ "score": id, "page": pos, "degrees": 90 }),
                                    ));
                                }
                                if ui
                                    .small_button(if hidden { "Show" } else { "Hide" })
                                    .clicked()
                                {
                                    run.push((
                                        "page.setHidden",
                                        json!({ "score": id, "page": pos, "hidden": !hidden }),
                                    ));
                                }
                                if pos > 1 && ui.small_button("↑").clicked() {
                                    run.push((
                                        "page.move",
                                        json!({ "score": id, "from": pos, "to": pos - 1 }),
                                    ));
                                }
                                if pos < n && ui.small_button("↓").clicked() {
                                    run.push((
                                        "page.move",
                                        json!({ "score": id, "from": pos, "to": pos + 1 }),
                                    ));
                                }
                                if n > 1
                                    && ui
                                        .small_button("✕")
                                        .on_hover_text("Delete from the score (the file keeps it)")
                                        .clicked()
                                {
                                    run.push((
                                        "page.delete",
                                        json!({ "score": id, "pages": [pos] }),
                                    ));
                                }
                            });
                        }
                    });
                ui.separator();
                ui.label(RichText::new("Bookmarks").strong().color(tokens.text));
                for b in score
                    .get("bookmarks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let bid = b.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
                    let name = b.get("name").and_then(Value::as_str).unwrap_or("");
                    ui.horizontal(|ui| {
                        if ui.link(name).clicked() {
                            run.push(("view.bookmark.goto", json!({ "bookmark": bid })));
                        }
                        if ui.small_button("✕").clicked() {
                            run.push((
                                "score.bookmark.remove",
                                json!({ "score": id, "bookmark": bid }),
                            ));
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.bookmark)
                            .hint_text("Name")
                            .desired_width(180.0),
                    );
                    let ready = !self.bookmark.trim().is_empty() && reader_page.is_some();
                    if ui
                        .add_enabled(ready, egui::Button::new("Bookmark this page"))
                        .clicked()
                        && let Some(page) = reader_page
                    {
                        run.push((
                            "score.bookmark.add",
                            json!({ "score": id, "name": self.bookmark.trim(), "page": page }),
                        ));
                        self.bookmark.clear();
                    }
                });
            });
        self.open = open;
        if run.iter().any(|(id, _)| *id == "score.metadata.set") {
            // Reload from the engine next time, so the fields show what was stored.
            self.details.score.clear();
        }
        run
    }
}
