//! The library window (feature-map LIB-14, spec `docs/spec/library.md`): a sidebar of sections
//! (Library, Lists, Recordings, Practice, Search, Recently Deleted, Settings), and in Library a
//! grid of covers or a list, folders first, with a floating toolbar on the right and a menu on
//! each score. Every list comes from the engine
//! (`library.search`, `library.info`); covers are rendered off the frame (a worker thread on
//! native builds, one per frame on the web) and cached by score.

use std::collections::{BTreeSet, HashMap};

use egui::{
    Color32, ColorImage, Context, RichText, Sense, TextureHandle, TextureOptions, Ui, vec2,
};
use scorecraft_engine::{CoverJob, Session};
use serde_json::{Value, json};

use crate::icons::{self, Icon};
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
    /// Ask the player for a folder to sync through (SYNC-7).
    ChooseSyncFolder,
    /// Sync now and report what happened.
    SyncNow,
    /// Ask the player for PDFs and import them into the folder shown.
    ImportFiles,
    /// Ask the player for photos and make one score of them.
    ImportPhotos,
    /// Show this score's details to edit.
    EditDetails(String),
    /// Export this score as a PDF (`flatten` or `annotations`) and offer it.
    Share(String, &'static str),
    /// Make a list with this name and add these scores to it.
    NewListWith(String, Vec<String>),
}

/// The library's sections, as the sidebar lists them (owner, 2026-11-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Library,
    Lists,
    Recordings,
    Practice,
    Search,
    Trash,
    Settings,
}

impl Section {
    /// The sidebar's rows above the gap; Settings sits at the bottom.
    pub const MAIN: [(Section, &'static str, Icon); 6] = [
        (Section::Library, "Library", Icon::Library),
        (Section::Lists, "Lists", Icon::Lists),
        (Section::Recordings, "Recordings", Icon::Recordings),
        (Section::Practice, "Practice", Icon::Practice),
        (Section::Search, "Search", Icon::Search),
        (Section::Trash, "Recently Deleted", Icon::Trash),
    ];
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
    /// The name typed for a new placeholder.
    placeholder_name: String,
    /// A score just added to a list brought the library's page range with it: the list, the
    /// entry, and the range offered for tweaking (1-based).
    pub range_notice: Option<RangeNotice>,
    /// Search inside the scores' PDFs and notes (LIB-6) rather than their details.
    pub full_text: bool,
    /// Recently Deleted is shown instead of the library (LIB-10).
    pub show_trash: bool,
    /// From `library.info`: folders in Recently Deleted, and scores still filed in one.
    deleted_folders: Value,
    stranded: Value,
    /// From `library.scores` with `trashed`, while Recently Deleted is shown.
    trashed: Value,
    /// The section the sidebar shows.
    pub section: Section,
    /// Taps select scores instead of opening them; the scores selected.
    pub selecting: bool,
    pub selected: BTreeSet<String>,
    /// The name typed for a new folder.
    new_folder: String,
}

/// The quiet notice after adding a score whose library entry has a page range.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeNotice {
    pub list: String,
    pub entry: String,
    pub first: u64,
    pub last: u64,
}

impl RangeNotice {
    /// From `list.entry.add`'s answer, when the score brought a range.
    pub fn from_added(list: &str, added: &Value) -> Option<RangeNotice> {
        let r = added.get("pageRange")?;
        Some(RangeNotice {
            list: list.to_owned(),
            entry: added.get("entry")?.as_str()?.to_owned(),
            first: r.get("first")?.as_u64()?,
            last: r.get("last")?.as_u64()?,
        })
    }
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
            placeholder_name: String::new(),
            range_notice: None,
            full_text: false,
            show_trash: false,
            deleted_folders: Value::Null,
            stranded: Value::Null,
            trashed: Value::Null,
            section: Section::Library,
            selecting: false,
            selected: BTreeSet::new(),
            new_folder: String::new(),
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
        // Search looks through the whole library; Library through the folder shown.
        let folder = match self.section {
            Section::Search => None,
            _ => self.folder.clone(),
        };
        let ask = (
            format!(
                "{}{}",
                if self.full_text { "\u{1}" } else { "" },
                self.query
            ),
            folder.clone(),
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
            if let (Some(f), Some(o)) = (&folder, p.as_object_mut()) {
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
        self.deleted_folders = info.get("deletedFolders").cloned().unwrap_or(Value::Null);
        self.stranded = info.get("inDeletedFolders").cloned().unwrap_or(Value::Null);
        self.trashed = if self.show_trash {
            session
                .dispatch("library.scores", &json!({ "trashed": true }))
                .ok()
                .and_then(|v| v.get("scores").cloned())
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
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

    /// The left sidebar (owner, 2026-11-01): compact rows for Library, Lists, Recordings,
    /// Practice, Search and Recently Deleted, then a gap and Settings at the bottom. When a
    /// score is open, a row on top goes back to it.
    pub fn sidebar(&mut self, ui: &mut Ui, open_score: Option<&str>) -> Vec<LibraryAction> {
        let mut out = Vec::new();
        ui.add_space(6.0);
        if let Some(title) = open_score {
            if icons::row(ui, Icon::Back, title, false)
                .on_hover_text("Back to the open score")
                .clicked()
            {
                out.push(LibraryAction::Command(
                    "ui.library.show",
                    json!({ "on": false }),
                ));
            }
            ui.separator();
        }
        for (section, label, icon) in Section::MAIN {
            if icons::row(ui, icon, label, self.section == section).clicked() {
                self.go(section);
            }
        }
        let rest = ui.available_height() - 40.0;
        if rest > 0.0 {
            ui.add_space(rest);
        }
        ui.separator();
        if icons::row(
            ui,
            Icon::Settings,
            "Settings",
            self.section == Section::Settings,
        )
        .clicked()
        {
            self.go(Section::Settings);
        }
        out
    }

    /// Shows a section of the sidebar.
    pub fn go(&mut self, section: Section) {
        if self.section != section {
            self.section = section;
            self.show_trash = section == Section::Trash;
            self.selecting = false;
            self.selected.clear();
            self.dirty = true;
        }
    }

    /// The section shown, filling `ui` (Settings is the shell's own page). Returns what the
    /// player chose.
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
        match self.section {
            Section::Library => self.library_section(ui, session, tokens, &mut out),
            Section::Search => self.search_section(ui, session, tokens, &mut out),
            Section::Lists => self.lists_panel(ui, tokens, &mut out),
            Section::Trash => {
                heading(ui, tokens, "Recently Deleted");
                self.trash_ui(ui, tokens, &mut out);
            }
            Section::Recordings => {
                heading(ui, tokens, "Recordings");
                ui.label(
                    RichText::new(
                        "Recordings made while practising will be kept here. Not built yet.",
                    )
                    .color(tokens.text),
                );
            }
            Section::Practice => {
                heading(ui, tokens, "Practice");
                ui.label(RichText::new("Coming later.").color(tokens.text));
            }
            Section::Settings => {}
        }
        out
    }

    /// The library: the folder shown, its folders and scores, with the floating toolbar on
    /// the right (owner, 2026-11-01; enScore's screenshot 3 for the arrangement).
    fn library_section(
        &mut self,
        ui: &mut Ui,
        session: &mut Session,
        tokens: &Tokens,
        out: &mut Vec<LibraryAction>,
    ) {
        let folders = self.folder_list();
        let here = self
            .folder
            .as_ref()
            .and_then(|cur| folders.iter().find(|f| &f.0 == cur).cloned());
        ui.horizontal(|ui| {
            if let Some((_, name, parent)) = &here {
                if icons::small_button_sized(ui, Icon::Back, "Up a folder", 28.0, false).clicked() {
                    self.folder = parent.clone();
                    self.dirty = true;
                }
                heading(ui, tokens, name);
            } else {
                heading(ui, tokens, "Library");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.floating_toolbar(ui, session, tokens, out);
            });
        });
        self.stranded_prompt(ui, tokens, out);
        if self.selecting {
            self.selection_bar(ui, tokens, &folders, out);
        }
        if !self.query.trim().is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Filtered by “{}”", self.query.trim()))
                        .color(tokens.text),
                );
                if ui.small_button("Clear").clicked() {
                    self.query.clear();
                }
            });
        }
        ui.add_space(4.0);
        let subfolders: Vec<(String, String)> = folders
            .iter()
            .filter(|f| f.2 == self.folder)
            .map(|f| (f.0.clone(), f.1.clone()))
            .collect();
        self.results_ui(ui, session, tokens, &subfolders, &folders, out);
    }

    /// Search (owner, 2026-11-01): the whole library by details or inside the scores. Searching
    /// IMSLP for new scores comes later.
    fn search_section(
        &mut self,
        ui: &mut Ui,
        session: &mut Session,
        tokens: &Tokens,
        out: &mut Vec<LibraryAction>,
    ) {
        heading(ui, tokens, "Search");
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Title, composer, tags, key…")
                    .desired_width(320.0),
            );
            if self.query.is_empty() {
                r.request_focus();
            }
            if ui
                .selectable_label(self.full_text, "Inside scores")
                .on_hover_text("Search the words in the PDFs and practice notes")
                .clicked()
            {
                self.full_text = !self.full_text;
            }
        });
        ui.label(
            RichText::new("Searching IMSLP for new scores is not built yet.")
                .weak()
                .italics(),
        );
        ui.add_space(4.0);
        if self.query.trim().is_empty() {
            return;
        }
        let folders = self.folder_list();
        self.results_ui(ui, session, tokens, &[], &folders, out);
    }

    /// Right-anchored floating toolbar: More, Filter, Sort, New Folder, Import.
    fn floating_toolbar(
        &mut self,
        ui: &mut Ui,
        session: &mut Session,
        tokens: &Tokens,
        out: &mut Vec<LibraryAction>,
    ) {
        egui::Frame::NONE
            .fill(tokens.toolbar_bg)
            .stroke(egui::Stroke::new(1.0, tokens.text.gamma_multiply(0.15)))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(6, 3))
            .shadow(egui::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: Color32::from_black_alpha(40),
            })
            .show(ui, |ui| {
                // Right to left: the last button added shows first.
                let import = icons::labelled(ui, Icon::Plus, "Import");
                egui::Popup::menu(&import).show(|ui| {
                    if icons::labelled(ui, Icon::Score, "PDF files…").clicked() {
                        out.push(LibraryAction::ImportFiles);
                        ui.close();
                    }
                    if !cfg!(target_arch = "wasm32") {
                        if icons::labelled(ui, Icon::Folder, "A folder of scores…").clicked() {
                            out.push(LibraryAction::ImportFolder);
                            ui.close();
                        }
                        if ui.button("Photos as one score…").clicked() {
                            out.push(LibraryAction::ImportPhotos);
                            ui.close();
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new("New blank score").weak());
                    for (paper, label) in [
                        ("single", "Manuscript paper"),
                        ("grand", "Piano manuscript paper"),
                        ("tab", "Tablature paper"),
                    ] {
                        if ui.button(label).clicked() {
                            out.push(LibraryAction::Command(
                                "score.newManuscript",
                                json!({ "paper": paper }),
                            ));
                            ui.close();
                        }
                    }
                });
                let nf = icons::labelled(ui, Icon::NewFolder, "New Folder");
                let parent = self.folder.clone();
                egui::Popup::menu(&nf)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.new_folder)
                                .hint_text("Folder name")
                                .desired_width(180.0),
                        );
                        r.request_focus();
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (ui.button("Create").clicked() || enter)
                            && !self.new_folder.trim().is_empty()
                        {
                            let mut p = json!({ "name": self.new_folder.trim() });
                            if let (Some(f), Some(o)) = (&parent, p.as_object_mut()) {
                                o.insert("parent".into(), json!(f));
                            }
                            out.push(LibraryAction::Command("library.folder.new", p));
                            self.new_folder.clear();
                            ui.close();
                        }
                    });
                ui.separator();
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
                let sorts = [
                    ("title", "Title"),
                    ("composer", "Composer"),
                    ("added", "Date added"),
                    ("opened", "Recently opened"),
                ];
                let label = sorts.iter().find(|s| s.0 == by).map_or("Title", |s| s.1);
                let sr =
                    icons::labelled(ui, Icon::Sort, &format!("By {label}")).on_hover_text("Sort");
                egui::Popup::menu(&sr).show(|ui| {
                    for (id, label) in sorts {
                        if icons::labelled_selected(ui, Icon::Check, label, by == id).clicked() {
                            out.push(LibraryAction::Command(
                                "library.sort.set",
                                json!({ "by": id }),
                            ));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.selectable_label(desc, "Reverse order").clicked() {
                        out.push(LibraryAction::Command(
                            "library.sort.set",
                            json!({ "descending": !desc }),
                        ));
                        ui.close();
                    }
                });
                let filtering = !self.query.trim().is_empty();
                let fr = icons::small_button_sized(ui, Icon::Filter, "Filter", 28.0, filtering);
                egui::Popup::menu(&fr)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.query)
                                .hint_text("Show only scores matching…")
                                .desired_width(220.0),
                        );
                        r.request_focus();
                        ui.checkbox(&mut self.full_text, "Inside scores");
                    });
                let more = icons::small_button_sized(ui, Icon::More, "More", 28.0, false);
                egui::Popup::menu(&more).show(|ui| {
                    if ui
                        .button(if self.selecting {
                            "Done selecting"
                        } else {
                            "Select"
                        })
                        .clicked()
                    {
                        self.selecting = !self.selecting;
                        self.selected.clear();
                        ui.close();
                    }
                    ui.separator();
                    if ui.radio(self.grid, "Show as covers").clicked() {
                        out.push(LibraryAction::Command(
                            "ui.library.view",
                            json!({ "view": "grid" }),
                        ));
                        ui.close();
                    }
                    if ui.radio(!self.grid, "Show as a list").clicked() {
                        out.push(LibraryAction::Command(
                            "ui.library.view",
                            json!({ "view": "list" }),
                        ));
                        ui.close();
                    }
                    if !cfg!(target_arch = "wasm32") {
                        ui.separator();
                        sync_buttons(ui, session, out);
                    }
                });
            });
    }

    /// While selecting: how many, and what to do with them.
    fn selection_bar(
        &mut self,
        ui: &mut Ui,
        tokens: &Tokens,
        folders: &[(String, String, Option<String>)],
        out: &mut Vec<LibraryAction>,
    ) {
        let ids: Vec<String> = self.selected.iter().cloned().collect();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(match ids.len() {
                    0 => "Tap scores to select them".to_owned(),
                    1 => "1 selected".to_owned(),
                    n => format!("{n} selected"),
                })
                .color(tokens.text),
            );
            ui.add_enabled_ui(!ids.is_empty(), |ui| {
                ui.menu_button("Add to list", |ui| {
                    self.list_menu(ui, &ids, out);
                });
                ui.menu_button("Move to", |ui| {
                    move_menu(ui, folders, &ids, out);
                });
                if ui.button("Delete").clicked() {
                    for id in &ids {
                        out.push(LibraryAction::Command(
                            "library.score.delete",
                            json!({ "score": id }),
                        ));
                    }
                    self.selected.clear();
                }
            });
            if ui.button("Done").clicked() {
                self.selecting = false;
                self.selected.clear();
            }
        });
    }

    /// The lists, to add `scores` to one (or to a new list).
    fn list_menu(&mut self, ui: &mut Ui, scores: &[String], out: &mut Vec<LibraryAction>) {
        let lists = self.list_names();
        if lists.is_empty() {
            ui.label(RichText::new("No lists yet").weak());
        }
        for (list, name) in lists {
            if ui.button(name).clicked() {
                for id in scores {
                    out.push(LibraryAction::Command(
                        "list.entry.add",
                        json!({ "list": list, "score": id }),
                    ));
                }
                ui.close();
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_list)
                    .hint_text("New list")
                    .desired_width(120.0),
            );
            if ui.button("Add").clicked() && !self.new_list.trim().is_empty() {
                out.push(LibraryAction::NewListWith(
                    self.new_list.trim().to_owned(),
                    scores.to_vec(),
                ));
                self.new_list.clear();
                ui.close();
            }
        });
    }

    fn list_names(&self) -> Vec<(String, String)> {
        self.lists
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| {
                Some((
                    l.get("id")?.as_str()?.to_owned(),
                    l.get("name")?.as_str()?.to_owned(),
                ))
            })
            .collect()
    }

    /// Every folder: id, name, parent.
    fn folder_list(&self) -> Vec<(String, String, Option<String>)> {
        self.folders
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
            .collect()
    }

    /// Folders, then the scores of the last search, as covers or a list.
    fn results_ui(
        &mut self,
        ui: &mut Ui,
        session: &mut Session,
        tokens: &Tokens,
        subfolders: &[(String, String)],
        folders: &[(String, String, Option<String>)],
        out: &mut Vec<LibraryAction>,
    ) {
        if let Some(e) = self.results.get("error").and_then(Value::as_str) {
            ui.colored_label(tokens.error, e);
            return;
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
        if pending > 0 {
            ui.label(
                RichText::new(format!("{count} so far; reading {pending} more…"))
                    .color(tokens.text),
            );
            ui.ctx().request_repaint();
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if self.grid {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
                        for (id, name) in subfolders {
                            self.folder_card(ui, tokens, id, name, out);
                        }
                        for s in &scores {
                            self.card(ui, session, s, tokens, folders, out);
                        }
                    });
                    if scores.is_empty() && subfolders.is_empty() {
                        ui.label(RichText::new("No scores here.").color(tokens.text));
                    }
                    return;
                }
                for (id, name) in subfolders {
                    let r = icons::row(ui, Icon::Folder, name, false);
                    if r.clicked() {
                        self.folder = Some(id.clone());
                        self.dirty = true;
                    }
                    folder_menu(&r, id, out);
                }
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
                            let id = s.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
                            let title = s.get("title").and_then(Value::as_str).unwrap_or("");
                            let chosen = self.selected.contains(&id);
                            let r = if self.selecting {
                                ui.selectable_label(chosen, title)
                            } else {
                                ui.link(title)
                            };
                            if r.clicked() {
                                self.tap_score(&id, out);
                            }
                            self.score_menu(&r, &id, folders, out);
                            ui.label(s.get("composer").and_then(Value::as_str).unwrap_or(""));
                            ui.label(
                                s.get("visiblePages")
                                    .map(ToString::to_string)
                                    .unwrap_or_default(),
                            );
                            ui.label(day(s.get("addedMs").and_then(Value::as_u64).unwrap_or(0)));
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
            });
    }

    /// A tap on a score: opens it, or while selecting, toggles it.
    fn tap_score(&mut self, id: &str, out: &mut Vec<LibraryAction>) {
        if self.selecting {
            if !self.selected.remove(id) {
                self.selected.insert(id.to_owned());
            }
        } else {
            out.push(LibraryAction::Open(id.to_owned()));
        }
    }

    /// A score's right-click menu, in the owner's order (2026-11-01): Edit Details, Add to
    /// list, Move to, Select, Share, Print, Delete.
    fn score_menu(
        &mut self,
        r: &egui::Response,
        id: &str,
        folders: &[(String, String, Option<String>)],
        out: &mut Vec<LibraryAction>,
    ) {
        r.context_menu(|ui| {
            if icons::labelled(ui, Icon::Info, "Edit Details").clicked() {
                out.push(LibraryAction::EditDetails(id.to_owned()));
                ui.close();
            }
            ui.menu_button("Add to list", |ui| {
                self.list_menu(ui, &[id.to_owned()], out);
            });
            ui.menu_button("Move to", |ui| {
                move_menu(ui, folders, &[id.to_owned()], out);
            });
            if icons::labelled(ui, Icon::Check, "Select").clicked() {
                self.selecting = true;
                self.selected.insert(id.to_owned());
                ui.close();
            }
            ui.menu_button("Share", |ui| {
                if ui.button("PDF with ink drawn in…").clicked() {
                    out.push(LibraryAction::Share(id.to_owned(), "flatten"));
                    ui.close();
                }
                if ui.button("PDF with ink as annotations…").clicked() {
                    out.push(LibraryAction::Share(id.to_owned(), "annotations"));
                    ui.close();
                }
            });
            ui.add_enabled(false, egui::Button::new("Print"))
                .on_disabled_hover_text("Printing is not built yet");
            ui.separator();
            if icons::labelled(ui, Icon::Trash, "Delete").clicked() {
                out.push(LibraryAction::Command(
                    "library.score.delete",
                    json!({ "score": id }),
                ));
                ui.close();
            }
        });
    }

    /// A folder as a card: its icon and name.
    fn folder_card(
        &mut self,
        ui: &mut Ui,
        tokens: &Tokens,
        id: &str,
        name: &str,
        out: &mut Vec<LibraryAction>,
    ) {
        let (rect, r) = ui.allocate_exact_size(CARD, Sense::click());
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            let art = egui::Rect::from_center_size(
                egui::pos2(rect.center().x, rect.top() + (CARD.y - 44.0) / 2.0),
                vec2(110.0, 110.0),
            );
            icons::paint(p, art, tokens.accent, Icon::Folder);
            p.text(
                egui::pos2(rect.center().x, rect.top() + CARD.y - 32.0),
                egui::Align2::CENTER_CENTER,
                clip(name),
                egui::FontId::proportional(13.0),
                tokens.text,
            );
            if r.hovered() {
                p.rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(1.5, tokens.accent),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if r.clicked() {
            self.folder = Some(id.to_owned());
            self.dirty = true;
        }
        folder_menu(&r, id, out);
    }

    fn card(
        &mut self,
        ui: &mut Ui,
        session: &mut Session,
        s: &Value,
        tokens: &Tokens,
        folders: &[(String, String, Option<String>)],
        out: &mut Vec<LibraryAction>,
    ) {
        let Some(id) = s.get("id").and_then(Value::as_str).map(str::to_owned) else {
            return;
        };
        let (rect, r) = ui.allocate_exact_size(CARD, Sense::click());
        if !ui.is_rect_visible(rect) {
            return;
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
        p.text(
            egui::pos2(rect.center().x, cover_rect.bottom() + 12.0),
            egui::Align2::CENTER_CENTER,
            clip(title),
            egui::FontId::proportional(13.0),
            tokens.text,
        );
        p.text(
            egui::pos2(rect.center().x, cover_rect.bottom() + 30.0),
            egui::Align2::CENTER_CENTER,
            clip(composer),
            egui::FontId::proportional(11.0),
            tokens.text.gamma_multiply(0.7),
        );
        let chosen = self.selected.contains(&id);
        if r.hovered() || chosen {
            p.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(if chosen { 2.5 } else { 1.5 }, tokens.accent),
                egui::StrokeKind::Inside,
            );
        }
        if self.selecting {
            let tick = egui::Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(22.0, 22.0));
            p.circle_filled(
                tick.center(),
                11.0,
                if chosen {
                    tokens.accent
                } else {
                    Color32::from_white_alpha(200)
                },
            );
            if chosen {
                icons::paint(p, tick.shrink(3.0), Color32::WHITE, Icon::Check);
            }
        }
        let r = r.on_hover_text(format!("{title}\n{composer}"));
        if r.clicked() {
            self.tap_score(&id, out);
        }
        self.score_menu(&r, &id, folders, out);
    }
}

/// An icon in a row of text, `side` points square.
fn glyph(ui: &mut Ui, icon: Icon, color: Color32, side: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    icons::paint(ui.painter(), rect, color, icon);
}

/// A section's heading.
fn heading(ui: &mut Ui, tokens: &Tokens, text: &str) {
    ui.label(RichText::new(text).size(22.0).strong().color(tokens.text));
}

/// A name cut to fit under a card.
fn clip(t: &str) -> String {
    if t.chars().count() > 22 {
        format!("{}…", t.chars().take(21).collect::<String>())
    } else {
        t.to_owned()
    }
}

/// The folders to move `scores` to: the library's top level, then every folder by path.
fn move_menu(
    ui: &mut Ui,
    folders: &[(String, String, Option<String>)],
    scores: &[String],
    out: &mut Vec<LibraryAction>,
) {
    let mut send = |folder: Value, ui: &mut Ui| {
        for id in scores {
            out.push(LibraryAction::Command(
                "library.score.move",
                json!({ "score": id, "folder": folder }),
            ));
        }
        ui.close();
    };
    if icons::labelled(ui, Icon::Library, "Library (no folder)").clicked() {
        send(Value::Null, ui);
    }
    for (id, _, _) in folders {
        if icons::labelled(ui, Icon::Folder, &folder_path(folders, id)).clicked() {
            send(json!(id), ui);
        }
    }
}

/// "Parent / Child" for a folder.
fn folder_path(folders: &[(String, String, Option<String>)], id: &str) -> String {
    let mut parts = Vec::new();
    let mut cur = Some(id.to_owned());
    // A cycle in bad data can't loop forever.
    for _ in 0..=folders.len() {
        let Some(c) = cur else { break };
        let Some(f) = folders.iter().find(|f| f.0 == c) else {
            break;
        };
        parts.push(f.1.clone());
        cur = f.2.clone();
    }
    parts.reverse();
    parts.join(" / ")
}

/// A folder's right-click menu.
fn folder_menu(r: &egui::Response, id: &str, out: &mut Vec<LibraryAction>) {
    r.context_menu(|ui| {
        if icons::labelled(ui, Icon::Trash, "Delete folder").clicked() {
            out.push(LibraryAction::Command(
                "library.folder.delete",
                json!({ "folder": id }),
            ));
            ui.close();
        }
    });
}

impl LibraryView {
    /// The setlists (LIST-1, LIST-4): pick one to see its entries, reorder or remove them, add a
    /// break, and play from any entry. Scores are added from their covers' menus.
    fn lists_panel(&mut self, ui: &mut Ui, tokens: &Tokens, out: &mut Vec<LibraryAction>) {
        heading(ui, tokens, "Lists");
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
                .add_enabled_ui(!entries.is_empty(), |ui| {
                    icons::labelled(ui, Icon::Play, "Play")
                })
                .inner
                .clicked()
            {
                out.push(LibraryAction::Command("list.play", json!({ "list": list })));
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.placeholder_name)
                    .hint_text("Interval")
                    .desired_width(110.0),
            );
            if ui
                .button("Add placeholder")
                .on_hover_text("A blank A4 page with this name on it, to write on")
                .clicked()
            {
                let name = self.placeholder_name.trim();
                let label = if name.is_empty() { "Interval" } else { name };
                out.push(LibraryAction::Command(
                    "list.entry.addPlaceholder",
                    json!({ "list": list, "label": label }),
                ));
                self.placeholder_name.clear();
            }
            if ui.button("Delete list").clicked() {
                out.push(LibraryAction::Command(
                    "list.delete",
                    json!({ "list": list }),
                ));
                self.open_list = None;
            }
        });
        self.range_notice_ui(ui, &list, out);
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
                        if icons::small_button(ui, Icon::Play, "Play from here").clicked() {
                            out.push(LibraryAction::Command(
                                "list.play",
                                json!({ "list": list, "entry": i + 1 }),
                            ));
                        }
                        if i > 0 && icons::small_button(ui, Icon::Up, "Move up").clicked() {
                            out.push(LibraryAction::Command(
                                "list.entry.move",
                                json!({ "list": list, "from": i + 1, "to": i }),
                            ));
                        }
                        if i + 1 < n && icons::small_button(ui, Icon::Down, "Move down").clicked() {
                            out.push(LibraryAction::Command(
                                "list.entry.move",
                                json!({ "list": list, "from": i + 1, "to": i + 2 }),
                            ));
                        }
                        if icons::small_button(ui, Icon::Close, "Remove from the list").clicked() {
                            out.push(LibraryAction::Command(
                                "list.entry.remove",
                                json!({ "list": list, "entry": entry }),
                            ));
                        }
                    });
                }
            });
    }

    /// Scores still filed in a folder in Recently Deleted (deleted on another device while this
    /// one filed them): ask whether to restore the folder.
    fn stranded_prompt(&self, ui: &mut Ui, tokens: &Tokens, out: &mut Vec<LibraryAction>) {
        let Some(first) = self.stranded.as_array().and_then(|a| a.first()) else {
            return;
        };
        let n = self.stranded.as_array().map_or(0, Vec::len);
        let (Some(folder), Some(path)) = (
            first.get("restore").and_then(Value::as_str),
            first.get("restorePath").and_then(Value::as_str),
        ) else {
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(if n == 1 {
                    format!("1 score is filed in “{path}”, which is in Recently Deleted.")
                } else {
                    format!("{n} scores are filed in folders in Recently Deleted (“{path}”…).")
                })
                .color(tokens.text),
            );
            if ui.button("Restore folder").clicked() {
                out.push(LibraryAction::Command(
                    "library.trash.restore",
                    json!({ "folder": folder }),
                ));
            }
        });
    }

    /// Recently Deleted (LIB-10): deleted folders and scores, each with Restore, kept 30 days.
    fn trash_ui(&self, ui: &mut Ui, tokens: &Tokens, out: &mut Vec<LibraryAction>) {
        let folders: Vec<Value> = self.deleted_folders.as_array().cloned().unwrap_or_default();
        let scores: Vec<Value> = self.trashed.as_array().cloned().unwrap_or_default();
        ui.label(
            RichText::new("Kept for 30 days, then removed for good. Restore to change them.")
                .weak(),
        );
        if folders.is_empty() && scores.is_empty() {
            ui.label(RichText::new("Nothing here.").color(tokens.text));
            return;
        }
        if ui.button("Empty Recently Deleted").clicked() {
            out.push(LibraryAction::Command("library.trash.empty", json!({})));
        }
        egui::ScrollArea::vertical()
            .id_salt("trash")
            .auto_shrink(false)
            .show(ui, |ui| {
                for f in &folders {
                    let (Some(id), Some(path)) = (
                        f.get("id").and_then(Value::as_str),
                        f.get("path").and_then(Value::as_str),
                    ) else {
                        continue;
                    };
                    ui.horizontal(|ui| {
                        glyph(ui, Icon::Folder, tokens.text, 18.0);
                        ui.label(path);
                        if let Some(at) = f.get("deletedMs").and_then(Value::as_u64) {
                            ui.label(RichText::new(format!("deleted {}", day(at))).weak());
                        }
                        if ui.small_button("Restore").clicked() {
                            out.push(LibraryAction::Command(
                                "library.trash.restore",
                                json!({ "folder": id }),
                            ));
                        }
                    });
                }
                for s in &scores {
                    let Some(id) = s.get("id").and_then(Value::as_str) else {
                        continue;
                    };
                    let title = s.get("title").and_then(Value::as_str).unwrap_or("?");
                    ui.horizontal(|ui| {
                        glyph(ui, Icon::Score, tokens.text, 18.0);
                        ui.label(title);
                        if ui.small_button("Restore").clicked() {
                            out.push(LibraryAction::Command(
                                "library.trash.restore",
                                json!({ "score": id }),
                            ));
                        }
                    });
                }
            });
    }

    /// The quiet notice that an added score kept its library page range, with the range to
    /// tweak for this entry only.
    fn range_notice_ui(&mut self, ui: &mut Ui, list: &str, out: &mut Vec<LibraryAction>) {
        let Some(n) = self.range_notice.as_mut() else {
            return;
        };
        if n.list != list {
            return;
        }
        let mut done = false;
        ui.horizontal_wrapped(|ui| {
            glyph(ui, Icon::Info, ui.visuals().weak_text_color(), 16.0);
            ui.label(RichText::new("Kept the score's page range: pages").weak());
            ui.add(egui::DragValue::new(&mut n.first).range(1..=n.last.max(1)));
            ui.label(RichText::new("to").weak());
            ui.add(egui::DragValue::new(&mut n.last).range(n.first..=9999));
            if ui.small_button("Apply").clicked() {
                out.push(LibraryAction::Command(
                    "list.entry.pageRange.set",
                    json!({ "list": n.list, "entry": n.entry, "first": n.first, "last": n.last }),
                ));
                done = true;
            }
            if ui.small_button("All pages").clicked() {
                out.push(LibraryAction::Command(
                    "list.entry.pageRange.set",
                    json!({ "list": n.list, "entry": n.entry }),
                ));
                done = true;
            }
            if ui.small_button("OK").clicked() {
                done = true;
            }
        });
        if done {
            self.range_notice = None;
        }
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

/// Sync through a shared folder: choose one, or sync now with what happened last on hover.
pub(crate) fn sync_buttons(ui: &mut Ui, session: &mut Session, out: &mut Vec<LibraryAction>) {
    let status = session
        .dispatch("sync.status", &json!({}))
        .unwrap_or(Value::Null);
    match status.get("backend").and_then(Value::as_str) {
        Some(folder) => {
            let waiting = status
                .get("waitingFiles")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let mut tip = format!("Syncing through {folder}");
            if waiting > 0 {
                tip.push_str(&format!(
                    "\n{waiting} score files are still on their way from other devices"
                ));
            }
            if let Some(p) = status
                .get("problems")
                .and_then(Value::as_array)
                .and_then(|a| a.last())
            {
                tip.push_str(&format!("\nLast problem: {}", p.as_str().unwrap_or("")));
            }
            if icons::labelled(ui, Icon::Sync, "Sync now")
                .on_hover_text(tip)
                .clicked()
            {
                out.push(LibraryAction::SyncNow);
            }
            if ui.button("Sync through another folder…").clicked() {
                out.push(LibraryAction::ChooseSyncFolder);
            }
        }
        None => {
            if ui
                .button("Sync…")
                .on_hover_text(
                    "Sync this library with your other devices through a folder they all \
                     reach: one Google Drive, OneDrive, Dropbox or iCloud Drive keeps in sync",
                )
                .clicked()
            {
                out.push(LibraryAction::ChooseSyncFolder);
            }
        }
    }
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
