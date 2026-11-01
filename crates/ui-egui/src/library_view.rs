//! The library window (feature-map LIB-14, spec `docs/spec/library.md`): a search box and the
//! sort above a grid of covers or a list, folders first. Every list comes from the engine
//! (`library.search`, `library.info`); covers are rendered off the frame (a worker thread on
//! native builds, one per frame on the web) and cached by score.

use std::collections::HashMap;

use egui::{
    Color32, ColorImage, Context, RichText, Sense, TextureHandle, TextureOptions, Ui, vec2,
};
use scorecraft_engine::{CoverJob, Session};
use serde_json::{Value, json};

use crate::theme::Tokens;

/// The longest side of a cover, in pixels.
const COVER_PX: u32 = 220;
/// A grid card's size, in points.
const CARD: egui::Vec2 = egui::Vec2::new(150.0, 230.0);
/// Covers asked for at once.
const MAX_PENDING: usize = 4;
/// Covers kept as textures.
const MAX_COVERS: usize = 600;

/// What the window asks the shell to do.
#[derive(Debug, Clone, PartialEq)]
pub enum LibraryAction {
    /// Open this library score in the reader.
    Open(String),
    /// Run this command (sort, view).
    Command(&'static str, Value),
    /// Ask the player for a folder and import it.
    ImportFolder,
}

enum Cover {
    Pending,
    Ready(TextureHandle),
    Failed,
}

/// Renders covers away from the frame.
struct Worker {
    #[cfg(not(target_arch = "wasm32"))]
    send: Option<std::sync::mpsc::Sender<CoverJob>>,
    #[cfg(not(target_arch = "wasm32"))]
    recv: Option<std::sync::mpsc::Receiver<(String, Option<scorecraft_pdf::Bitmap>)>>,
    /// Jobs waiting (the web build renders one per frame).
    queue: Vec<CoverJob>,
}

impl Worker {
    fn new() -> Worker {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (send, jobs) = std::sync::mpsc::channel::<CoverJob>();
            let (done, recv) = std::sync::mpsc::channel();
            let started = std::thread::Builder::new()
                .name("scorecraft-covers".into())
                .spawn(move || {
                    while let Ok(job) = jobs.recv() {
                        let bm = job.render(COVER_PX).ok();
                        if done.send((job.score.to_string(), bm)).is_err() {
                            break;
                        }
                    }
                })
                .is_ok();
            Worker {
                send: started.then_some(send),
                recv: started.then_some(recv),
                queue: Vec::new(),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Worker { queue: Vec::new() }
        }
    }

    fn submit(&mut self, job: CoverJob) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(s) = &self.send {
            // A worker that stopped leaves its job to the frame.
            if let Err(e) = s.send(job) {
                self.queue.push(e.0);
            }
            return;
        }
        self.queue.push(job);
    }

    /// Finished covers.
    fn done(&mut self) -> Vec<(String, Option<scorecraft_pdf::Bitmap>)> {
        let mut out = Vec::new();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(r) = &self.recv {
            while let Ok(x) = r.try_recv() {
                out.push(x);
            }
        }
        // Without a thread: one per frame.
        if let Some(job) = (!self.queue.is_empty()).then(|| self.queue.remove(0)) {
            out.push((job.score.to_string(), job.render(COVER_PX).ok()));
        }
        out
    }
}

pub struct LibraryView {
    pub query: String,
    pub grid: bool,
    /// The folder shown (its id), or the library root.
    pub folder: Option<String>,
    /// Results of the last search, and what it was asked with.
    results: Value,
    folders: Value,
    asked: Option<(String, Option<String>, Value)>,
    /// The library changed since the last search.
    pub dirty: bool,
    covers: HashMap<String, Cover>,
    pending: usize,
    worker: Worker,
    /// The setlist shown in the Lists panel (LIST-1), and what `list.get` said of it.
    pub open_list: Option<String>,
    list_detail: Value,
    lists: Value,
    new_list: String,
    /// Search inside the scores' PDFs and notes (LIB-6) rather than their details.
    pub full_text: bool,
}

impl Default for LibraryView {
    fn default() -> Self {
        LibraryView {
            query: String::new(),
            grid: true,
            folder: None,
            results: Value::Null,
            folders: Value::Null,
            asked: None,
            dirty: true,
            covers: HashMap::new(),
            pending: 0,
            worker: Worker::new(),
            open_list: None,
            list_detail: Value::Null,
            lists: Value::Null,
            new_list: String::new(),
            full_text: false,
        }
    }
}

impl LibraryView {
    pub fn new() -> LibraryView {
        LibraryView::default()
    }

    /// How many covers are ready to draw.
    pub fn ready_covers(&self) -> usize {
        self.covers
            .values()
            .filter(|c| matches!(c, Cover::Ready(_)))
            .count()
    }

    /// Forgets a score's cover (its pages changed).
    pub fn forget_covers(&mut self) {
        self.covers.clear();
    }

    fn refresh(&mut self, session: &mut Session) {
        let sort = session
            .dispatch("library.sort.set", &json!({}))
            .ok()
            .and_then(|v| v.get("sort").cloned())
            .unwrap_or(Value::Null);
        let ask = (
            format!(
                "{}{}",
                if self.full_text { "\u{1}" } else { "" },
                self.query
            ),
            self.folder.clone(),
            sort,
        );
        if !self.dirty && self.asked.as_ref() == Some(&ask) {
            return;
        }
        let mut more = false;
        self.results = if self.full_text && !self.query.trim().is_empty() {
            // A little indexing per frame; while files are waiting, ask again next frame.
            let r = session
                .dispatch(
                    "library.search.fullText",
                    &json!({ "query": self.query, "limit": 200, "budgetMs": 100 }),
                )
                .unwrap_or_else(|e| json!({ "error": e.to_string() }));
            more = r.get("pending").and_then(Value::as_u64).unwrap_or(0) > 0;
            r
        } else {
            let mut p = json!({ "query": self.query, "limit": 500 });
            if let (Some(f), Some(o)) = (&self.folder, p.as_object_mut()) {
                o.insert("folder".into(), json!(f));
            }
            session
                .dispatch("library.search", &p)
                .unwrap_or_else(|e| json!({ "error": e.to_string() }))
        };
        let info = session
            .dispatch("library.info", &json!({}))
            .unwrap_or(Value::Null);
        self.folders = info.get("folders").cloned().unwrap_or(Value::Null);
        self.lists = info.get("lists").cloned().unwrap_or(Value::Null);
        self.list_detail = match &self.open_list {
            Some(id) => session
                .dispatch("list.get", &json!({ "list": id }))
                .unwrap_or(Value::Null),
            None => Value::Null,
        };
        if self.list_detail.is_null() {
            self.open_list = None;
        }
        self.asked = Some(ask);
        self.dirty = more;
    }

    fn take_covers(&mut self, ctx: &Context) {
        for (id, bm) in self.worker.done() {
            self.pending = self.pending.saturating_sub(1);
            let cover = match bm {
                Some(bm) => {
                    let image = ColorImage::from_rgba_premultiplied(
                        [bm.width as usize, bm.height as usize],
                        bm.bytes(),
                    );
                    Cover::Ready(ctx.load_texture(
                        format!("cover-{id}"),
                        image,
                        TextureOptions::LINEAR,
                    ))
                }
                None => Cover::Failed,
            };
            self.covers.insert(id, cover);
        }
        if self.covers.len() > MAX_COVERS {
            self.covers.clear();
        }
        if self.pending > 0 {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    fn want_cover(&mut self, session: &mut Session, id: &str) {
        if self.covers.contains_key(id) || self.pending >= MAX_PENDING {
            return;
        }
        match session.cover_job(id) {
            Ok(job) => {
                self.worker.submit(job);
                self.pending += 1;
                self.covers.insert(id.to_owned(), Cover::Pending);
            }
            Err(_) => {
                self.covers.insert(id.to_owned(), Cover::Failed);
            }
        }
    }

    /// The window, filling `ui`. Returns what the player chose.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        session: &mut Session,
        tokens: &Tokens,
    ) -> Vec<LibraryAction> {
        let mut out = Vec::new();
        if session.library().is_none() {
            ui.label("No library is open.");
            return out;
        }
        let ctx = ui.ctx().clone();
        self.take_covers(&ctx);
        self.refresh(session);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search title, composer, tags, key…")
                    .desired_width(320.0),
            );
            if ui
                .selectable_label(self.full_text, "Inside scores")
                .on_hover_text("Search the words in the PDFs and practice notes")
                .clicked()
            {
                self.full_text = !self.full_text;
            }
            let sort = self
                .asked
                .as_ref()
                .map(|a| a.2.clone())
                .unwrap_or(Value::Null);
            let by = sort
                .get("by")
                .and_then(Value::as_str)
                .unwrap_or("title")
                .to_owned();
            let desc = sort
                .get("descending")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            egui::ComboBox::from_id_salt("sort")
                .selected_text(match by.as_str() {
                    "composer" => "Composer",
                    "added" => "Date added",
                    "opened" => "Recently opened",
                    _ => "Title",
                })
                .show_ui(ui, |ui| {
                    for (id, label) in [
                        ("title", "Title"),
                        ("composer", "Composer"),
                        ("added", "Date added"),
                        ("opened", "Recently opened"),
                    ] {
                        if ui.selectable_label(by == id, label).clicked() {
                            out.push(LibraryAction::Command(
                                "library.sort.set",
                                json!({ "by": id }),
                            ));
                        }
                    }
                });
            if ui.selectable_label(desc, "Reverse").clicked() {
                out.push(LibraryAction::Command(
                    "library.sort.set",
                    json!({ "descending": !desc }),
                ));
            }
            ui.separator();
            if ui.selectable_label(self.grid, "Covers").clicked() {
                out.push(LibraryAction::Command(
                    "ui.library.view",
                    json!({ "view": "grid" }),
                ));
            }
            if ui.selectable_label(!self.grid, "List").clicked() {
                out.push(LibraryAction::Command(
                    "ui.library.view",
                    json!({ "view": "list" }),
                ));
            }
            ui.separator();
            if ui.button("Import Folder…").clicked() {
                out.push(LibraryAction::ImportFolder);
            }
        });
        ui.add_space(6.0);
        let area = ui.available_rect_before_wrap();
        let panel = egui::Rect::from_min_size(area.min, vec2(260.0, area.height()));
        let mut panel_ui = ui.new_child(egui::UiBuilder::new().max_rect(panel));
        self.lists_panel(&mut panel_ui, tokens, &mut out);
        let rest = egui::Rect::from_min_max(egui::pos2(panel.right() + 12.0, area.top()), area.max);
        let mut rest_ui = ui.new_child(egui::UiBuilder::new().max_rect(rest));
        let ui = &mut rest_ui;
        // Folders inside the one shown, then a way back up.
        let folders: Vec<(String, String, Option<String>)> = self
            .folders
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| {
                Some((
                    f.get("id")?.as_str()?.to_owned(),
                    f.get("name")?.as_str()?.to_owned(),
                    f.get("parent").and_then(Value::as_str).map(str::to_owned),
                ))
            })
            .collect();
        ui.horizontal_wrapped(|ui| {
            if let Some(cur) = self.folder.clone() {
                let up = folders
                    .iter()
                    .find(|f| f.0 == cur)
                    .and_then(|f| f.2.clone());
                if ui.button("⬅ Up").clicked() {
                    self.folder = up;
                }
                let name = folders
                    .iter()
                    .find(|f| f.0 == cur)
                    .map(|f| f.1.clone())
                    .unwrap_or_default();
                ui.label(RichText::new(name).strong().color(tokens.text));
            }
            for (id, name, parent) in &folders {
                if *parent == self.folder && ui.button(format!("📁 {name}")).clicked() {
                    self.folder = Some(id.clone());
                }
            }
        });
        if let Some(e) = self.results.get("error").and_then(Value::as_str) {
            ui.colored_label(tokens.error, e);
            return out;
        }
        let scores: Vec<Value> = self
            .results
            .get("scores")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let count = self
            .results
            .get("count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let pending = self
            .results
            .get("pending")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        ui.label(
            RichText::new(match (count, pending) {
                (1, 0) => "1 score".to_owned(),
                (n, 0) => format!("{n} scores"),
                (n, p) => format!("{n} so far; reading {p} more…"),
            })
            .color(tokens.text),
        );
        if pending > 0 {
            ui.ctx().request_repaint();
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if self.grid {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
                        for s in &scores {
                            if let Some(a) = self.card(ui, session, s, tokens) {
                                out.push(a);
                            }
                        }
                    });
                } else {
                    egui::Grid::new("library-list")
                        .striped(true)
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.strong("Title");
                            ui.strong("Composer");
                            ui.strong("Pages");
                            ui.strong("Added");
                            ui.end_row();
                            for s in &scores {
                                let id =
                                    s.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
                                let title = s.get("title").and_then(Value::as_str).unwrap_or("");
                                if ui.link(title).clicked() {
                                    out.push(LibraryAction::Open(id));
                                }
                                ui.label(s.get("composer").and_then(Value::as_str).unwrap_or(""));
                                ui.label(
                                    s.get("visiblePages")
                                        .map(ToString::to_string)
                                        .unwrap_or_default(),
                                );
                                ui.label(day(s
                                    .get("addedMs")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(0)));
                                ui.end_row();
                                if let Some(sn) = s
                                    .get("snippet")
                                    .or_else(|| s.get("notes"))
                                    .and_then(Value::as_str)
                                {
                                    let page = s
                                        .get("filePage")
                                        .and_then(Value::as_u64)
                                        .map(|p| format!("p. {p}: "))
                                        .unwrap_or_default();
                                    ui.label(RichText::new(format!("{page}{sn}")).weak().italics());
                                    ui.end_row();
                                }
                            }
                        });
                }
            });
        out
    }

    /// The setlists (LIST-1, LIST-4): pick one to see its entries, reorder or remove them, add a
    /// break, and play from any entry. Scores are added from their covers' menus.
    fn lists_panel(&mut self, ui: &mut Ui, tokens: &Tokens, out: &mut Vec<LibraryAction>) {
        ui.label(RichText::new("Lists").strong().color(tokens.text));
        let lists: Vec<(String, String, u64)> = self
            .lists
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| {
                Some((
                    l.get("id")?.as_str()?.to_owned(),
                    l.get("name")?.as_str()?.to_owned(),
                    l.get("entries").and_then(Value::as_u64).unwrap_or(0),
                ))
            })
            .collect();
        for (id, name, n) in &lists {
            let open = self.open_list.as_deref() == Some(id.as_str());
            if ui.selectable_label(open, format!("{name} ({n})")).clicked() {
                self.open_list = if open { None } else { Some(id.clone()) };
                self.dirty = true;
            }
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_list)
                    .hint_text("New list")
                    .desired_width(150.0),
            );
            if ui.button("Add").clicked() && !self.new_list.trim().is_empty() {
                out.push(LibraryAction::Command(
                    "list.new",
                    json!({ "name": self.new_list.trim() }),
                ));
                self.new_list.clear();
            }
        });
        let Some(list) = self.open_list.clone() else {
            return;
        };
        ui.separator();
        let entries: Vec<Value> = self
            .list_detail
            .get("entries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!entries.is_empty(), egui::Button::new("▶ Play"))
                .clicked()
            {
                out.push(LibraryAction::Command("list.play", json!({ "list": list })));
            }
            if ui.button("Add break").clicked() {
                out.push(LibraryAction::Command(
                    "list.entry.addPlaceholder",
                    json!({ "list": list, "label": "Break" }),
                ));
            }
            if ui.button("Delete list").clicked() {
                out.push(LibraryAction::Command(
                    "list.delete",
                    json!({ "list": list }),
                ));
                self.open_list = None;
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("list-entries")
            .show(ui, |ui| {
                let n = entries.len();
                for (i, e) in entries.iter().enumerate() {
                    let entry = e.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
                    let label = e
                        .get("title")
                        .or_else(|| e.get("label"))
                        .and_then(Value::as_str)
                        .unwrap_or("?");
                    ui.horizontal(|ui| {
                        ui.label(format!("{}. {label}", i + 1));
                        if ui
                            .small_button("▶")
                            .on_hover_text("Play from here")
                            .clicked()
                        {
                            out.push(LibraryAction::Command(
                                "list.play",
                                json!({ "list": list, "entry": i + 1 }),
                            ));
                        }
                        if i > 0 && ui.small_button("↑").clicked() {
                            out.push(LibraryAction::Command(
                                "list.entry.move",
                                json!({ "list": list, "from": i + 1, "to": i }),
                            ));
                        }
                        if i + 1 < n && ui.small_button("↓").clicked() {
                            out.push(LibraryAction::Command(
                                "list.entry.move",
                                json!({ "list": list, "from": i + 1, "to": i + 2 }),
                            ));
                        }
                        if ui.small_button("✕").clicked() {
                            out.push(LibraryAction::Command(
                                "list.entry.remove",
                                json!({ "list": list, "entry": entry }),
                            ));
                        }
                    });
                }
            });
    }

    fn card(
        &mut self,
        ui: &mut Ui,
        session: &mut Session,
        s: &Value,
        tokens: &Tokens,
    ) -> Option<LibraryAction> {
        let id = s.get("id").and_then(Value::as_str)?.to_owned();
        let (rect, r) = ui.allocate_exact_size(CARD, Sense::click());
        if !ui.is_rect_visible(rect) {
            return None;
        }
        self.want_cover(session, &id);
        let p = ui.painter();
        let cover_rect = egui::Rect::from_min_size(rect.min, vec2(CARD.x, CARD.y - 44.0));
        match self.covers.get(&id) {
            Some(Cover::Ready(t)) => {
                let size = t.size_vec2();
                let k = (cover_rect.width() / size.x).min(cover_rect.height() / size.y);
                let img = egui::Rect::from_center_size(cover_rect.center(), size * k);
                p.image(
                    t.id(),
                    img,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                p.rect_stroke(
                    img,
                    0.0,
                    egui::Stroke::new(1.0, Color32::from_gray(160)),
                    egui::StrokeKind::Outside,
                );
            }
            _ => {
                p.rect_filled(cover_rect.shrink(8.0), 2.0, tokens.placeholder);
            }
        }
        let title = s.get("title").and_then(Value::as_str).unwrap_or("");
        let composer = s.get("composer").and_then(Value::as_str).unwrap_or("");
        let font = egui::FontId::proportional(13.0);
        let clip = |t: &str| -> String {
            if t.chars().count() > 22 {
                format!("{}…", t.chars().take(21).collect::<String>())
            } else {
                t.to_owned()
            }
        };
        p.text(
            egui::pos2(rect.center().x, cover_rect.bottom() + 12.0),
            egui::Align2::CENTER_CENTER,
            clip(title),
            font.clone(),
            tokens.text,
        );
        p.text(
            egui::pos2(rect.center().x, cover_rect.bottom() + 30.0),
            egui::Align2::CENTER_CENTER,
            clip(composer),
            egui::FontId::proportional(11.0),
            tokens.text.gamma_multiply(0.7),
        );
        if r.hovered() {
            p.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.5, tokens.accent),
                egui::StrokeKind::Inside,
            );
        }
        let r = r.on_hover_text(format!("{title}\n{composer}"));
        let mut add = None;
        if let Some(list) = self.open_list.clone() {
            r.context_menu(|ui| {
                if ui.button("Add to the open list").clicked() {
                    add = Some(LibraryAction::Command(
                        "list.entry.add",
                        json!({ "list": list, "score": id }),
                    ));
                    ui.close();
                }
            });
        }
        add.or_else(|| r.clicked().then_some(LibraryAction::Open(id)))
    }
}

/// A Unix-millisecond time as a date, `YYYY-MM-DD` (UTC).
pub fn day(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    // Civil-from-days (Howard Hinnant's algorithm), proleptic Gregorian.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_are_dates() {
        assert_eq!(day(0), "1970-01-01");
        assert_eq!(day(1_793_404_800_000), "2026-10-31");
        assert_eq!(day(951_782_400_000), "2000-02-29");
    }
}
