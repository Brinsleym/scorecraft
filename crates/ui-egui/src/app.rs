//! The ScoreCraft shell: toolbar, reader and shell commands. Platform-free; the desktop and web
//! apps supply a [`Platform`] and call [`App::ui`] every frame.

use egui::{Align, Color32, Context, Layout, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};
use scorecraft_engine::{EngineError, Session};
use serde_json::{Value, json};

use crate::reader::ReaderCanvas;
use crate::settings::Settings;
use crate::state::{Theme, UiState};
use crate::theme::Tokens;

/// What the shell needs from the platform.
pub trait Platform {
    /// Asks the user for a score file. `None` if cancelled or there is no picker.
    fn pick_score(&mut self) -> Option<String> {
        None
    }
    /// Holds or releases the screen awake.
    fn set_keep_awake(&mut self, on: bool) -> Result<(), String>;
    /// Stores the settings for the next launch.
    fn save_settings(&mut self, _settings: &Settings) {}
    /// Where the library lives (markings are kept there). `None` keeps a library in memory for
    /// the session only.
    fn library_dir(&self) -> Option<String> {
        None
    }
    /// Asks where to save an exported PDF, suggesting `name`. `None` if cancelled or there is no
    /// dialog.
    fn pick_export_path(&mut self, _name: &str) -> Option<String> {
        None
    }
    /// Asks for a folder to import. `None` if cancelled or there is no dialog.
    fn pick_folder(&mut self) -> Option<String> {
        None
    }
    /// Plays the metronome's click at (BPM, beats per bar), or stops it with `None`. An error
    /// says why there is no sound (the pulse still shows).
    fn set_click(&mut self, _click: Option<(f64, u8)>) -> Result<(), String> {
        Err("this build has no audio output".into())
    }
}

/// A platform that does nothing (tests, snapshots).
pub struct NoPlatform;

impl Platform for NoPlatform {
    fn set_keep_awake(&mut self, _on: bool) -> Result<(), String> {
        Ok(())
    }
}

/// The shell commands (`ui.*`, AGENTS.md rule 2). Listed in `shell-commands.toml` for parity.
pub const SHELL_COMMANDS: &[(&str, &str, &str)] = &[
    ("ui.view.fullScreen", "Full Screen", r#"{"on":bool?}"#),
    (
        "ui.power.keepAwake",
        "Keep Screen Awake",
        r#"{"on":bool=true}"#,
    ),
    ("ui.theme.set", "Theme", r#"{"theme":"light|dark"}"#),
    (
        "ui.chrome.toggle",
        "Show or Hide Controls",
        r#"{"on":bool?}"#,
    ),
    ("ui.library.show", "Library", r#"{"on":bool?}"#),
    ("ui.library.view", "Library View", r#"{"view":"grid|list"}"#),
];

pub struct App {
    pub session: Session,
    pub state: UiState,
    pub settings: Settings,
    reader: ReaderCanvas,
    platform: Box<dyn Platform>,
    /// Full-screen state last sent to the window.
    sent_full_screen: Option<bool>,
    /// The theme last applied to egui.
    applied_theme: Option<Theme>,
    /// The library window (LIB-14).
    library: crate::library_view::LibraryView,
    /// The Score panel: details, pages, bookmarks.
    score_panel: crate::score_panel::ScorePanel,
    /// `library.score.get` for the open score, while the Score panel is open; cleared by every
    /// command so the panel never shows stale pages.
    score_cache: Option<Value>,
    /// The click last asked of the platform, and why there is none, if there is none.
    sent_click: Option<Option<(f64, u8)>>,
    click_error: Option<String>,
}

fn bool_param(params: &Value, key: &str) -> Result<Option<bool>, EngineError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(EngineError::BadParams {
            command: "ui".into(),
            message: format!("`{key}` must be true or false"),
        }),
    }
}

impl App {
    pub fn new(platform: Box<dyn Platform>, settings: Settings) -> App {
        let mut session = Session::new();
        let mut state = UiState::new();
        // Scores open through the library, so markings are kept with them (ANNO-15).
        let opened = match platform.library_dir() {
            Some(dir) => {
                let id = if std::path::Path::new(&dir).exists() {
                    "library.open"
                } else {
                    "library.create"
                };
                session.dispatch(id, &json!({ "path": dir })).map(|_| ())
            }
            None => session.create_memory_library().map(|_| ()),
        };
        if let Err(e) = opened {
            state.message = Some(format!(
                "The library couldn't be opened ({e}); markings made now won't be kept."
            ));
            let _ = session.create_memory_library();
        }
        settings.apply_to_session(&mut session);
        settings.apply_markup(&mut session);
        state.theme = settings.theme;
        state.keep_awake.wanted = settings.keep_awake;
        state.library_grid = settings.library_grid;
        App {
            session,
            state,
            settings,
            reader: ReaderCanvas::new(),
            platform,
            sent_full_screen: None,
            applied_theme: None,
            library: crate::library_view::LibraryView::new(),
            score_panel: crate::score_panel::ScorePanel::new(),
            score_cache: None,
            sent_click: None,
            click_error: None,
        }
    }

    /// Opens a score file and applies the saved reader preferences.
    pub fn open_path(&mut self, path: &str) -> Result<Value, EngineError> {
        if self.session.library().is_some() {
            self.command("library.openFile", &json!({ "path": path }))
        } else {
            self.command("score.open", &json!({ "path": path }))
        }
    }

    /// After a score opens, by any route: fresh textures, the saved reader preferences, and
    /// remember it for the next launch.
    fn opened(&mut self, params: &Value) {
        self.reader.clear();
        self.settings.apply_to_reader(&mut self.session);
        if let Some(path) = params.get("path").and_then(Value::as_str) {
            self.settings.last_score = Some(path.to_owned());
        }
        self.save();
    }

    /// Whether every visible page has been rendered (screenshots wait for this).
    pub fn pages_ready(&mut self) -> bool {
        ReaderCanvas::pages_ready(&mut self.session)
    }

    fn save(&mut self) {
        self.settings.capture(&mut self.session, &self.state);
        self.platform.save_settings(&self.settings);
    }

    /// Runs a shell (`ui.*`) or engine command by id: what the toolbar, menus and the control
    /// channel call.
    pub fn command(&mut self, id: &str, params: &Value) -> Result<Value, EngineError> {
        let r = if id.starts_with("ui.") {
            self.shell(id, params)
        } else {
            self.session.dispatch(id, params)
        };
        if r.is_ok() && id == "list.play" {
            // A list opens its first score in the reader.
            self.reader.clear();
            self.state.library = false;
            self.library.dirty = true;
            return r;
        }
        if let (Ok(_), true) = (&r, id == "score.open" || id == "library.openFile") {
            self.opened(params);
            self.state.library = false;
            self.library.dirty = true;
            return r;
        }
        if r.is_ok() {
            // Imports, edits and sorts change what the library window lists.
            self.library.dirty = true;
            self.score_cache = None;
        }
        // Markup settings are kept between launches; gestures and queries are not settings.
        let markup_setting = (id.starts_with("annotate.")
            && !matches!(
                id,
                "annotate.stroke"
                    | "annotate.state"
                    | "annotate.page.list"
                    | "annotate.ruler.set"
                    | "annotate.mode.set"
            )
            && !id.starts_with("annotate.select.")
            && !id.starts_with("annotate.text.")
            && !id.starts_with("annotate.stamp.")
            && !id.starts_with("annotate.layer."))
            || id == "input.pen.only"
            || id == "input.touch.lock"
            || id == "library.sort.set"
            || id == "ui.library.view";
        let changes_prefs = id.starts_with("view.layout")
            || id.starts_with("view.turn.halfPage")
            || id.starts_with("view.turn.alternating")
            || id == "view.zoom.landscapeFit"
            || id == "input.pedal.map"
            || id == "ui.theme.set"
            || id == "ui.power.keepAwake"
            || markup_setting;
        if r.is_ok() && changes_prefs {
            self.save();
        }
        r
    }

    fn shell(&mut self, id: &str, params: &Value) -> Result<Value, EngineError> {
        if !(params.is_null() || params.is_object()) {
            return Err(EngineError::BadParams {
                command: id.chars().take(64).collect(),
                message: "params must be an object".into(),
            });
        }
        match id {
            "ui.view.fullScreen" => {
                let on = bool_param(params, "on")?.unwrap_or(!self.state.full_screen);
                self.state.full_screen = on;
                self.state.chrome = !on;
            }
            "ui.power.keepAwake" => {
                self.state.keep_awake.wanted = bool_param(params, "on")?.unwrap_or(true);
            }
            "ui.theme.set" => {
                let t = params
                    .get("theme")
                    .and_then(Value::as_str)
                    .and_then(Theme::from_id)
                    .ok_or_else(|| EngineError::BadParams {
                        command: id.into(),
                        message: "`theme` must be light or dark".into(),
                    })?;
                self.state.theme = t;
            }
            "ui.chrome.toggle" => {
                self.state.chrome = bool_param(params, "on")?.unwrap_or(!self.state.chrome);
            }
            "ui.library.show" => {
                self.state.library = bool_param(params, "on")?.unwrap_or(!self.state.library);
            }
            "ui.library.view" => {
                self.state.library_grid = match params.get("view").and_then(Value::as_str) {
                    Some("grid") => true,
                    Some("list") => false,
                    _ => {
                        return Err(EngineError::BadParams {
                            command: id.into(),
                            message: "`view` must be grid or list".into(),
                        });
                    }
                };
            }
            _ => return Err(EngineError::UnknownCommand(id.chars().take(128).collect())),
        }
        serde_json::to_value(&self.state).map_err(|e| EngineError::Io(e.to_string()))
    }

    /// Keeps the platform's click in step with the metronome (PRAC-1).
    fn sync_click(&mut self) {
        let st = self
            .session
            .dispatch("practice.metronome.state", &json!({}))
            .unwrap_or(Value::Null);
        let want = (st.get("running").and_then(Value::as_bool) == Some(true)).then(|| {
            (
                st.get("bpm").and_then(Value::as_f64).unwrap_or(100.0),
                st.get("beats")
                    .and_then(Value::as_u64)
                    .and_then(|b| u8::try_from(b).ok())
                    .unwrap_or(4),
            )
        });
        if self.sent_click == Some(want) {
            return;
        }
        self.sent_click = Some(want);
        match self.platform.set_click(want) {
            Ok(()) => self.click_error = None,
            Err(e) if want.is_some() => self.click_error = Some(e),
            Err(_) => {}
        }
    }

    /// Keeps the screen awake while wanted and a score is open (READ-9).
    fn sync_keep_awake(&mut self) {
        let want = self.state.keep_awake.wanted && self.session.has_score();
        if want == self.state.keep_awake.active {
            return;
        }
        match self.platform.set_keep_awake(want) {
            Ok(()) => {
                self.state.keep_awake.active = want;
                self.state.keep_awake.error = None;
            }
            Err(e) => {
                // Don't retry every frame: record it and stop wanting until asked again.
                self.state.keep_awake.error = Some(e);
                self.state.keep_awake.wanted = false;
            }
        }
    }

    /// One frame of the whole shell, in the window's root `ui`.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let tokens = Tokens::for_theme(self.state.theme);
        if self.applied_theme != Some(self.state.theme) {
            tokens.apply(self.state.theme, &ctx);
            self.applied_theme = Some(self.state.theme);
        }
        if self.sent_full_screen != Some(self.state.full_screen) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.state.full_screen));
            self.sent_full_screen = Some(self.state.full_screen);
        }
        self.sync_keep_awake();
        self.sync_click();
        self.shortcuts(&ctx);
        self.dropped_files(&ctx);

        if self.state.chrome {
            egui::Panel::top("toolbar")
                .exact_size(tokens.toolbar_height)
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(tokens.toolbar_bg)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show(ui, |ui| self.toolbar(ui, &tokens));
        }
        let markup = self
            .session
            .dispatch("annotate.state", &json!({}))
            .unwrap_or(Value::Null);
        if self.state.chrome
            && self.session.has_score()
            && markup.get("mode").and_then(Value::as_bool) == Some(true)
        {
            egui::Panel::top("markup")
                .exact_size(tokens.toolbar_height)
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(tokens.toolbar_bg)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show(ui, |ui| self.markup_bar(ui, &tokens, &markup));
        }
        if let Some(msg) = self.state.message.clone() {
            egui::Panel::bottom("message")
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(tokens.toolbar_bg)
                        .inner_margin(egui::Margin::same(8)),
                )
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(msg).color(tokens.error));
                        if ui.button("Dismiss").clicked() {
                            self.state.message = None;
                        }
                    });
                });
        }
        self.score_window(&ctx, &tokens);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(tokens.reader_bg))
            .show(ui, |ui| {
                if self.state.library || !self.session.has_score() {
                    if self.session.library().is_some() {
                        self.library_window(ui, &tokens);
                    } else {
                        self.empty(ui, &tokens);
                    }
                    return;
                }
                let keys = !ctx.egui_wants_keyboard_input();
                let area = ui.available_rect_before_wrap();
                let out = self.reader.show(ui, &mut self.session, &tokens, keys);
                self.metronome_pulse(ui, area, &tokens);
                if out.toggle_chrome {
                    self.state.chrome = !self.state.chrome;
                }
                if let Some(e) = out.error {
                    self.state.message = Some(e);
                }
            });
    }

    fn library_window(&mut self, ui: &mut Ui, tokens: &Tokens) {
        self.library.grid = self.state.library_grid;
        egui::Frame::NONE
            .fill(tokens.toolbar_bg)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                let actions = self.library.show(ui, &mut self.session, tokens);
                for a in actions {
                    match a {
                        crate::library_view::LibraryAction::Open(id) => {
                            let r = self.command("score.open", &json!({ "score": id }));
                            self.report(r);
                        }
                        crate::library_view::LibraryAction::Command(id, p) => {
                            let r = self.command(id, &p);
                            self.report(r);
                        }
                        crate::library_view::LibraryAction::ImportFolder => {
                            if let Some(path) = self.platform.pick_folder() {
                                let r = self.command(
                                    "library.import.folder",
                                    &json!({ "path": path, "keepFolders": true }),
                                );
                                match r {
                                    Ok(v) => self.state.message = Some(import_report(&v)),
                                    Err(e) => self.state.message = Some(e.to_string()),
                                }
                            }
                        }
                    }
                }
            });
    }

    /// The Score panel, while open over a library score.
    fn score_window(&mut self, ctx: &Context, tokens: &Tokens) {
        let Some(id) = self.session.library_score_id() else {
            self.score_panel.open = false;
            return;
        };
        if !self.score_panel.open {
            return;
        }
        if self.score_cache.is_none() {
            self.score_cache = self
                .session
                .dispatch("library.score.get", &json!({ "score": id }))
                .ok();
        }
        let Some(score) = self.score_cache.clone() else {
            return;
        };
        // The page on screen, as a position in the score's pages.
        let shown = self
            .session
            .reader_state()
            .and_then(|st| st.get("page").and_then(Value::as_u64));
        let reader_page = shown.and_then(|n| {
            score
                .get("visiblePages")
                .and_then(Value::as_array)
                .and_then(|v| v.get(usize::try_from(n).ok()?.checked_sub(1)?))
                .and_then(Value::as_u64)
                .map(|i| i + 1)
        });
        let run = self.score_panel.show(ctx, tokens, &score, reader_page);
        for (cmd, p) in run {
            let r = self.command(cmd, &p);
            self.report(r);
        }
    }

    /// The metronome's light pulse along the top edge of the reader (PRAC-1): bright on each
    /// beat, brighter and in the accent colour on the first of the bar, fading through the beat.
    fn metronome_pulse(&mut self, ui: &Ui, area: Rect, tokens: &Tokens) {
        let Ok(st) = self
            .session
            .dispatch("practice.metronome.state", &json!({}))
        else {
            return;
        };
        let Some(beat) = st.get("beat").filter(|b| b.is_object()) else {
            return;
        };
        let phase = beat.get("phase").and_then(Value::as_f64).unwrap_or(1.0) as f32;
        let accent = beat.get("accent").and_then(Value::as_bool).unwrap_or(false);
        let fade = (1.0 - phase * 2.5).clamp(0.0, 1.0);
        if fade > 0.0 {
            let colour = if accent {
                tokens.accent
            } else {
                Color32::from_gray(140)
            };
            let h = if accent { 10.0 } else { 6.0 };
            let bar = Rect::from_min_size(area.min, vec2(area.width(), h));
            ui.painter()
                .rect_filled(bar, 0.0, colour.gamma_multiply(fade));
        }
        // Wake for the fade and the next beat.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(if fade > 0.0 {
                16
            } else {
                40
            }));
    }

    fn metronome_menu(&mut self, ui: &mut Ui, run: &mut Vec<(&'static str, Value)>) {
        let st = self
            .session
            .dispatch("practice.metronome.state", &json!({}))
            .unwrap_or(Value::Null);
        let running = st.get("running").and_then(Value::as_bool).unwrap_or(false);
        let bpm = st.get("bpm").and_then(Value::as_f64).unwrap_or(100.0);
        let beats = st.get("beats").and_then(Value::as_u64).unwrap_or(4);
        ui.menu_button(format!("♩ {bpm:.0}"), |ui| {
            if ui
                .selectable_label(running, if running { "Stop" } else { "Start" })
                .clicked()
            {
                run.push(("practice.metronome.toggle", json!({})));
            }
            if let Some(e) = &self.click_error {
                ui.label(RichText::new(format!("No click: {e}")).weak());
            }
            if ui.button("Tap tempo").clicked() {
                run.push(("practice.metronome.tap", json!({})));
            }
            let mut b = bpm;
            if ui
                .add(egui::Slider::new(&mut b, 20.0..=300.0).text("BPM"))
                .changed()
            {
                run.push(("practice.metronome.set", json!({ "bpm": b.round() })));
            }
            ui.horizontal(|ui| {
                for m in [2u64, 3, 4, 6] {
                    if ui.radio(beats == m, m.to_string()).clicked() {
                        run.push(("practice.metronome.set", json!({ "beats": m })));
                    }
                }
            });
        });
    }

    fn markup_bar(&mut self, ui: &mut Ui, tokens: &Tokens, markup: &Value) {
        let mut run = Vec::new();
        let mut changed = false;
        let mut show_ruler = self.settings.show_ruler_button;
        crate::markup_bar::bar(
            ui,
            tokens,
            markup,
            self.session.library_score_id().as_deref(),
            &mut self.reader.ink,
            crate::markup_bar::BarSettings {
                show_ruler_button: &mut show_ruler,
                changed: &mut changed,
            },
            &mut run,
        );
        if changed {
            self.settings.show_ruler_button = show_ruler;
            self.save();
        }
        for (id, p) in run {
            let r = self.command(id, &p);
            self.report(r);
        }
    }

    fn report(&mut self, r: Result<Value, EngineError>) {
        match r {
            Ok(_) => self.state.message = None,
            Err(e) => self.state.message = Some(e.to_string()),
        }
    }

    /// Shell shortcuts; turn keys go to the reader's pedal map instead.
    fn shortcuts(&mut self, ctx: &Context) {
        let (open, full, esc) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::O),
                i.key_pressed(egui::Key::F11),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if open {
            self.pick_and_open();
        }
        if full {
            let r = self.command("ui.view.fullScreen", &json!({}));
            self.report(r);
        }
        if esc && self.state.full_screen {
            let r = self.command("ui.view.fullScreen", &json!({ "on": false }));
            self.report(r);
        }
    }

    fn dropped_files(&mut self, ctx: &Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            let r = self.open_path(&path.to_string_lossy());
            self.report(r);
        }
    }

    /// Exports the open score with its visible layers' ink (EXP-2).
    fn export(&mut self, mode: &str) {
        let name = self
            .settings
            .last_score
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .map(|s| format!("{} (marked).pdf", s.to_string_lossy()))
            .unwrap_or_else(|| "Score (marked).pdf".to_owned());
        if let Some(path) = self.platform.pick_export_path(&name) {
            let r = self.command("score.export.pdf", &json!({ "path": path, "mode": mode }));
            self.report(r);
        }
    }

    fn pick_and_open(&mut self) {
        if let Some(path) = self.platform.pick_score() {
            let r = self.open_path(&path);
            self.report(r);
        }
    }

    fn empty(&mut self, ui: &mut Ui, tokens: &Tokens) {
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                ui.label(
                    RichText::new("Open a score (Ctrl+O) or drop a PDF here")
                        .size(20.0)
                        .color(Color32::from_gray(230)),
                );
            },
        );
        let _ = tokens;
    }

    fn icon_button(
        &mut self,
        ui: &mut Ui,
        tokens: &Tokens,
        label: &str,
        selected: bool,
        icon: Icon,
    ) -> bool {
        let style = tokens.icon_button();
        let size = Vec2::splat(tokens.button);
        let r = craft_ui::buttons::IconButton::new(label, size, &style)
            .selected(selected)
            .show(ui, |ui, rect, colour| paint_icon(ui, rect, colour, icon))
            .on_hover_text(label);
        r.clicked()
    }

    fn toolbar(&mut self, ui: &mut Ui, tokens: &Tokens) {
        let st = self.session.reader_state().unwrap_or(Value::Null);
        let s = |k: &str| st.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let layout = s("layout");
        let mode = s("mode");
        let fit = st
            .pointer("/zoom/fit")
            .and_then(Value::as_str)
            .unwrap_or("page")
            .to_owned();
        let zoom = st
            .pointer("/zoom/factor")
            .and_then(Value::as_f64)
            .unwrap_or(1.0);
        let has = self.session.has_score();
        let mut run: Vec<(&str, Value)> = Vec::new();
        ui.horizontal_centered(|ui| {
            if self.icon_button(ui, tokens, "Open Score…", false, Icon::Open) {
                self.pick_and_open();
            }
            if self.session.library().is_some()
                && self.icon_button(ui, tokens, "Library", self.state.library, Icon::Library)
            {
                run.push(("ui.library.show", json!({})));
            }
            if self.session.library_score_id().is_some()
                && ui
                    .selectable_label(self.score_panel.open, "Score")
                    .on_hover_text("Details, pages and bookmarks")
                    .clicked()
            {
                self.score_panel.open = !self.score_panel.open;
            }
            if has {
                ui.menu_button("Export", |ui| {
                    if ui.button("PDF with ink drawn in…").clicked() {
                        self.export("flatten");
                        ui.close();
                    }
                    if ui.button("PDF with ink as annotations…").clicked() {
                        self.export("annotations");
                        ui.close();
                    }
                });
            }
            ui.add_enabled_ui(has, |ui| {
                if self.icon_button(ui, tokens, "Previous Page", false, Icon::Previous) {
                    run.push(("view.turn.previous", json!({})));
                }
                let page = st.get("page").and_then(Value::as_u64).unwrap_or(0);
                let count = st.get("pageCount").and_then(Value::as_u64).unwrap_or(0);
                ui.label(
                    RichText::new(format!("{page} / {count}"))
                        .size(16.0)
                        .color(tokens.text),
                );
                if self.icon_button(ui, tokens, "Next Page", false, Icon::Next) {
                    run.push(("view.turn.next", json!({})));
                }
                ui.separator();
                if self.icon_button(ui, tokens, "Single Page", layout == "single", Icon::Single) {
                    run.push(("view.layout.single", json!({})));
                }
                if self.icon_button(ui, tokens, "Two Pages", layout == "twoPage", Icon::Two) {
                    run.push(("view.layout.twoPage", json!({})));
                }
                if layout == "twoPage" {
                    let fa = st
                        .get("firstAlone")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if self.icon_button(ui, tokens, "First Page Alone", fa, Icon::FirstAlone) {
                        run.push(("view.layout.twoPage", json!({ "firstAlone": !fa })));
                    }
                }
                ui.separator();
                let half = mode == "halfPage";
                if self.icon_button(ui, tokens, "Half-Page Turns", half, Icon::Half) {
                    // Turning it on snaps the split to the gap between systems (G2).
                    run.push(("view.turn.halfPage", json!({ "on": !half, "snap": !half })));
                }
                let alt = mode == "alternating";
                if self.icon_button(ui, tokens, "Alternating Turns", alt, Icon::Alternate) {
                    run.push(("view.turn.alternating", json!({ "on": !alt })));
                }
                ui.separator();
                if self.icon_button(
                    ui,
                    tokens,
                    "Landscape Page Zoom",
                    fit == "width",
                    Icon::FitWidth,
                ) {
                    run.push(("view.zoom.landscapeFit", json!({ "on": fit != "width" })));
                }
                if self.icon_button(ui, tokens, "Zoom Out", false, Icon::ZoomOut) {
                    run.push(("view.zoom.set", json!({ "zoom": (zoom / 1.25).max(1.0) })));
                }
                if self.icon_button(ui, tokens, "Zoom In", false, Icon::ZoomIn) {
                    run.push(("view.zoom.set", json!({ "zoom": (zoom * 1.25).min(8.0) })));
                }
                ui.separator();
                self.metronome_menu(ui, &mut run);
                let marking = self
                    .session
                    .dispatch("annotate.state", &json!({}))
                    .ok()
                    .and_then(|m| m.get("mode").and_then(Value::as_bool))
                    .unwrap_or(false);
                if self.icon_button(ui, tokens, "Markup", marking, Icon::Markup) {
                    run.push(("annotate.mode.set", json!({ "on": !marking })));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let playing = self
                    .session
                    .dispatch("list.state", &json!({}))
                    .unwrap_or(Value::Null);
                if playing.get("playing").and_then(Value::as_bool) == Some(true) {
                    if ui.button("Stop list").clicked() {
                        run.push(("list.stop", json!({})));
                    }
                    let s = |k: &str| playing.get(k).and_then(Value::as_str).unwrap_or("");
                    let n = |k: &str| playing.get(k).and_then(Value::as_u64).unwrap_or(0);
                    let next = if s("next").is_empty() {
                        String::new()
                    } else {
                        format!(" · next: {}", s("next"))
                    };
                    ui.label(
                        RichText::new(format!(
                            "{} {}/{}: {}{next}",
                            s("name"),
                            n("entry"),
                            n("entries"),
                            s("title")
                        ))
                        .color(tokens.text),
                    );
                    ui.separator();
                }
                let dark = self.state.theme == Theme::Dark;
                if self.icon_button(ui, tokens, "Dark Theme", dark, Icon::Theme) {
                    run.push((
                        "ui.theme.set",
                        json!({ "theme": if dark { "light" } else { "dark" } }),
                    ));
                }
                let awake = self.state.keep_awake.wanted;
                let awake_label = match &self.state.keep_awake.error {
                    Some(e) => format!("Keep Screen Awake ({e})"),
                    None => "Keep Screen Awake".to_owned(),
                };
                if self.icon_button(ui, tokens, &awake_label, awake, Icon::Awake) {
                    run.push(("ui.power.keepAwake", json!({ "on": !awake })));
                }
                if self.icon_button(
                    ui,
                    tokens,
                    "Full Screen (F11)",
                    self.state.full_screen,
                    Icon::FullScreen,
                ) {
                    run.push(("ui.view.fullScreen", json!({})));
                }
            });
        });
        for (id, p) in run {
            let r = self.command(id, &p);
            self.report(r);
        }
    }
}

/// What an import did, in a sentence (IMP-6): imported, already in the library, ignored, and
/// each file that couldn't be read with why.
pub fn import_report(v: &Value) -> String {
    let n = |k: &str| match v.get(k) {
        Some(Value::Array(a)) => a.len() as u64,
        Some(x) => x.as_u64().unwrap_or(0),
        None => 0,
    };
    let plural = |n: u64, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    let mut parts = vec![format!(
        "Imported {}",
        plural(n("imported"), "score", "scores")
    )];
    if n("skipped") > 0 {
        parts.push(format!("{} already in the library", n("skipped")));
    }
    if n("ignored") > 0 {
        parts.push(format!(
            "{} not PDFs",
            plural(n("ignored"), "file", "files")
        ));
    }
    let failed: Vec<String> = v
        .get("failed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(5)
        .map(|f| {
            let path = f.get("path").and_then(Value::as_str).unwrap_or("?");
            let name = std::path::Path::new(path)
                .file_name()
                .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned());
            format!(
                "{name}: {}",
                f.get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unreadable")
            )
        })
        .collect();
    let mut out = parts.join("; ");
    if n("failed") > 0 {
        out.push_str(&format!(
            "; {} couldn't be read ({}{})",
            n("failed"),
            failed.join("; "),
            if n("failed") > 5 { "; …" } else { "" }
        ));
    }
    out.push('.');
    out
}

/// The toolbar's icons, painted with shapes so the shell ships no icon assets.
#[derive(Debug, Clone, Copy)]
enum Icon {
    Open,
    Previous,
    Next,
    Single,
    Two,
    FirstAlone,
    Half,
    Alternate,
    FitWidth,
    ZoomIn,
    ZoomOut,
    FullScreen,
    Awake,
    Theme,
    Markup,
    Library,
}

fn paint_icon(ui: &Ui, rect: Rect, c: Color32, icon: Icon) {
    let p = ui.painter();
    let r = rect.shrink(rect.width() * 0.28);
    let line = Stroke::new(1.8, c);
    let page = |x0: f32, x1: f32| Rect::from_min_max(pos2(x0, r.top()), pos2(x1, r.bottom()));
    match icon {
        Icon::Open => {
            let body = Rect::from_min_max(pos2(r.left(), r.top() + r.height() * 0.25), r.max);
            p.rect_stroke(body, 2.0, line, egui::StrokeKind::Middle);
            p.line_segment(
                [
                    pos2(r.left(), body.top()),
                    pos2(r.left() + r.width() * 0.4, r.top()),
                ],
                line,
            );
            p.line_segment(
                [
                    pos2(r.left() + r.width() * 0.4, r.top()),
                    pos2(r.center().x, body.top()),
                ],
                line,
            );
        }
        Icon::Previous | Icon::Next => {
            let (a, b) = if matches!(icon, Icon::Next) {
                (r.left(), r.right())
            } else {
                (r.right(), r.left())
            };
            let pts = vec![pos2(a, r.top()), pos2(b, r.center().y), pos2(a, r.bottom())];
            p.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
        }
        Icon::Single => {
            p.rect_stroke(
                page(r.left() + r.width() * 0.2, r.right() - r.width() * 0.2),
                1.0,
                line,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Two | Icon::FirstAlone => {
            let m = r.center().x;
            p.rect_stroke(page(r.left(), m - 1.0), 1.0, line, egui::StrokeKind::Middle);
            if matches!(icon, Icon::Two) {
                p.rect_stroke(
                    page(m + 1.0, r.right()),
                    1.0,
                    line,
                    egui::StrokeKind::Middle,
                );
            } else {
                p.text(
                    pos2(m + r.width() * 0.25, r.center().y),
                    egui::Align2::CENTER_CENTER,
                    "1",
                    egui::FontId::proportional(r.height() * 0.8),
                    c,
                );
            }
        }
        Icon::Half => {
            let pg = page(r.left() + r.width() * 0.2, r.right() - r.width() * 0.2);
            p.rect_stroke(pg, 1.0, line, egui::StrokeKind::Middle);
            p.rect_filled(
                Rect::from_min_max(pg.min, pos2(pg.right(), pg.center().y)),
                1.0,
                c.gamma_multiply(0.5),
            );
        }
        Icon::Alternate => {
            let m = r.center().x;
            p.rect_filled(page(r.left(), m - 1.0), 1.0, c.gamma_multiply(0.5));
            p.rect_stroke(
                page(m + 1.0, r.right()),
                1.0,
                line,
                egui::StrokeKind::Middle,
            );
        }
        Icon::FitWidth => {
            let y = r.center().y;
            p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], line);
            for (x, d) in [(r.left(), 1.0), (r.right(), -1.0)] {
                p.line_segment([pos2(x, y), pos2(x + d * 5.0, y - 5.0)], line);
                p.line_segment([pos2(x, y), pos2(x + d * 5.0, y + 5.0)], line);
            }
            p.line_segment([pos2(r.left(), r.top()), pos2(r.left(), r.bottom())], line);
            p.line_segment(
                [pos2(r.right(), r.top()), pos2(r.right(), r.bottom())],
                line,
            );
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            let rad = r.width() * 0.38;
            let ctr = r.center() - vec2(2.0, 2.0);
            p.circle_stroke(ctr, rad, line);
            p.line_segment([ctr + vec2(rad * 0.7, rad * 0.7), r.right_bottom()], line);
            p.line_segment(
                [ctr - vec2(rad * 0.5, 0.0), ctr + vec2(rad * 0.5, 0.0)],
                line,
            );
            if matches!(icon, Icon::ZoomIn) {
                p.line_segment(
                    [ctr - vec2(0.0, rad * 0.5), ctr + vec2(0.0, rad * 0.5)],
                    line,
                );
            }
        }
        Icon::FullScreen => {
            let k = r.width() * 0.35;
            for (corner, dx, dy) in [
                (r.left_top(), 1.0, 1.0),
                (r.right_top(), -1.0, 1.0),
                (r.left_bottom(), 1.0, -1.0),
                (r.right_bottom(), -1.0, -1.0),
            ] {
                p.line_segment([corner, corner + vec2(dx * k, 0.0)], line);
                p.line_segment([corner, corner + vec2(0.0, dy * k)], line);
            }
        }
        Icon::Awake => {
            let ctr = r.center();
            p.circle_stroke(ctr, r.width() * 0.22, line);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                p.line_segment(
                    [ctr + d * r.width() * 0.34, ctr + d * r.width() * 0.5],
                    line,
                );
            }
        }
        Icon::Library => {
            // Three book spines.
            let w = r.width() / 4.0;
            for i in 0..3 {
                let x = r.left() + i as f32 * (w + 2.0);
                p.rect_stroke(
                    Rect::from_min_max(
                        pos2(x, r.top() + (i % 2) as f32 * 3.0),
                        pos2(x + w, r.bottom()),
                    ),
                    1.0,
                    line,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::Markup => {
            // A pen, nib down to the left.
            let (a, b) = (pos2(r.left(), r.bottom()), pos2(r.right(), r.top()));
            p.line_segment([a + vec2(3.0, -3.0), b], Stroke::new(3.0, c));
            p.add(egui::Shape::convex_polygon(
                vec![a, a + vec2(6.0, 0.0), a + vec2(0.0, -6.0)],
                c,
                Stroke::NONE,
            ));
        }
        Icon::Theme => {
            let ctr = r.center();
            let rad = r.width() * 0.45;
            p.circle_stroke(ctr, rad, line);
            let half: Vec<egui::Pos2> = (0..=16)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 16.0;
                    ctr + vec2(a.cos(), a.sin()) * rad
                })
                .collect();
            p.add(egui::Shape::convex_polygon(half, c, Stroke::NONE));
        }
    }
    let _ = Sense::hover();
}

#[cfg(test)]
mod tests {
    use super::*;
    use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

    fn app_with_score(pages: usize) -> (App, std::path::PathBuf) {
        // A directory per call: tests run in parallel and must not share a file.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("scorecraft-ui-{}-{n}-{pages}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("score.pdf");
        std::fs::write(
            &path,
            PdfBuilder::new().pages(pages, PageSpec::default()).build(),
        )
        .unwrap();
        let mut app = App::new(Box::new(NoPlatform), Settings::default());
        app.open_path(&path.to_string_lossy()).unwrap();
        (app, path)
    }

    /// Runs `frames` frames of the shell headless at 1280 × 800.
    fn frames(app: &mut App, ctx: &Context, frames: usize, events: Vec<egui::Event>) {
        for i in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                events: if i == 0 { events.clone() } else { Vec::new() },
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| app.ui(ui));
            // No renderer here: drop the texture uploads on purpose.
            out.textures_delta.clear();
        }
    }

    #[test]
    fn shell_commands_match_the_parity_list() {
        let toml_text = include_str!("../shell-commands.toml");
        let v: toml::Value = toml::from_str(toml_text).unwrap();
        let listed: Vec<&str> = v["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        let ours: Vec<&str> = SHELL_COMMANDS.iter().map(|c| c.0).collect();
        assert_eq!(listed, ours);
    }

    #[test]
    fn shell_commands_change_state_and_reject_bad_params() {
        let mut app = App::new(Box::new(NoPlatform), Settings::default());
        let r = app.command("ui.view.fullScreen", &json!({})).unwrap();
        assert_eq!(
            (r["fullScreen"].as_bool(), r["chrome"].as_bool()),
            (Some(true), Some(false))
        );
        let r = app.command("ui.chrome.toggle", &json!({})).unwrap();
        assert_eq!(r["chrome"], true);
        let r = app
            .command("ui.theme.set", &json!({ "theme": "dark" }))
            .unwrap();
        assert_eq!(r["theme"], "dark");
        let r = app
            .command("ui.power.keepAwake", &json!({ "on": false }))
            .unwrap();
        assert_eq!(r["keepAwake"]["wanted"], false);
        for (id, p) in [
            ("ui.theme.set", json!({ "theme": "neon" })),
            ("ui.view.fullScreen", json!({ "on": 1 })),
            ("ui.view.fullScreen", json!([1])),
        ] {
            assert_eq!(
                app.command(id, &p).unwrap_err().code(),
                "bad_params",
                "{id}"
            );
        }
        assert_eq!(
            app.command("ui.nope", &json!({})).unwrap_err().code(),
            "unknown_command"
        );
    }

    #[test]
    fn a_score_opened_by_command_gets_the_window_viewport() {
        let (mut app, _) = app_with_score(2);
        let (_, second) = app_with_score(3);
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.25);
        frames(&mut app, &ctx, 2, Vec::new());
        let r = app.command("score.open", &json!({ "path": second.to_string_lossy() }));
        assert!(r.is_ok(), "{r:?}");
        frames(&mut app, &ctx, 1, Vec::new());
        let st = app.session.reader_state().unwrap();
        assert_eq!(st["pageCount"], 3);
        assert_eq!(st["viewport"]["pixelsPerPoint"], 1.25, "{}", st["viewport"]);
        assert_eq!(
            app.settings.last_score.as_deref(),
            Some(&*second.to_string_lossy())
        );
    }

    /// Runs one frame per entry of `script` at 1280 × 800, each with its events, 16 ms apart.
    fn script(app: &mut App, ctx: &Context, start: f64, script: Vec<Vec<egui::Event>>) {
        for (k, events) in script.into_iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                time: Some(start + k as f64 * 0.016),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| app.ui(ui));
            out.textures_delta.clear();
        }
    }

    fn touch(id: u64, phase: egui::TouchPhase, x: f32, y: f32, pen: bool) -> egui::Event {
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(id),
            phase,
            pos: pos2(x, y),
            force: pen.then_some(0.6),
        }
    }

    /// A drag from (`x0`, `y`) to (`x1`, `y`) by touch `id`, one sample per frame.
    fn drag(id: u64, x0: f32, x1: f32, y: f32, pen: bool) -> Vec<Vec<egui::Event>> {
        use egui::TouchPhase::{End, Move, Start};
        let mut v = vec![vec![touch(id, Start, x0, y, pen)]];
        for k in 1..8 {
            let x = x0 + (x1 - x0) * k as f32 / 8.0;
            v.push(vec![touch(id, Move, x, y, pen)]);
        }
        v.push(vec![touch(id, End, x1, y, pen)]);
        v
    }

    fn strokes_on_page(app: &mut App) -> usize {
        app.session
            .dispatch("annotate.page.list", &json!({}))
            .unwrap()["annotations"]
            .as_array()
            .unwrap()
            .len()
    }

    fn markup_app() -> (App, Context) {
        let (mut app, _) = app_with_score(3);
        let ctx = Context::default();
        frames(&mut app, &ctx, 2, Vec::new());
        app.command("annotate.mode.set", &json!({ "on": true }))
            .unwrap();
        frames(&mut app, &ctx, 1, Vec::new());
        (app, ctx)
    }

    #[test]
    fn a_pen_draws_in_markup_mode_and_a_finger_only_with_draw_with_touch() {
        let (mut app, ctx) = markup_app();
        script(&mut app, &ctx, 1.0, drag(7, 400.0, 700.0, 300.0, true));
        script(&mut app, &ctx, 2.0, vec![Vec::new(), Vec::new()]);
        assert_eq!(strokes_on_page(&mut app), 1, "the pen drew");
        assert_eq!(app.session.reader_state().unwrap()["page"], 1);
        // Without Draw with Touch, a finger's drag is left to the reader (egui-winit turns it
        // into pointer events there): no ink.
        script(&mut app, &ctx, 3.0, drag(8, 900.0, 300.0, 400.0, false));
        script(&mut app, &ctx, 4.0, vec![Vec::new(), Vec::new()]);
        assert_eq!(strokes_on_page(&mut app), 1, "the finger drew nothing");
        // With it, the finger draws.
        app.command("input.pen.only", &json!({ "on": false }))
            .unwrap();
        script(&mut app, &ctx, 5.0, drag(10, 450.0, 700.0, 500.0, false));
        script(&mut app, &ctx, 6.0, vec![Vec::new()]);
        assert_eq!(strokes_on_page(&mut app), 2, "{:?}", app.state.message);
    }

    #[test]
    fn ink_20_a_hand_leaning_while_the_pen_draws_does_nothing() {
        use egui::TouchPhase::{End, Move, Start};
        let (mut app, ctx) = markup_app();
        let mut s = vec![vec![touch(7, Start, 400.0, 300.0, true)]];
        // The palm lands while the pen is down, drags and lifts.
        s.push(vec![
            touch(7, Move, 450.0, 300.0, true),
            touch(9, Start, 1100.0, 500.0, false),
        ]);
        s.push(vec![
            touch(7, Move, 500.0, 300.0, true),
            touch(9, Move, 600.0, 520.0, false),
        ]);
        s.push(vec![touch(9, End, 300.0, 540.0, false)]);
        s.push(vec![touch(7, Move, 550.0, 300.0, true)]);
        s.push(vec![touch(7, End, 600.0, 300.0, true)]);
        s.push(Vec::new());
        script(&mut app, &ctx, 1.0, s);
        assert_eq!(app.session.reader_state().unwrap()["page"], 1, "no turn");
        assert_eq!(strokes_on_page(&mut app), 1, "the stroke is unaffected");
    }

    #[test]
    fn ink_17_18_the_mouse_draws_only_with_draw_with_touch() {
        let (mut app, ctx) = markup_app();
        let button = |pressed, x| egui::Event::PointerButton {
            pos: pos2(x, 300.0),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mouse = |start: f64, app: &mut App| {
            let mut s = vec![vec![
                egui::Event::PointerMoved(pos2(400.0, 300.0)),
                button(true, 400.0),
            ]];
            for k in 1..6 {
                s.push(vec![egui::Event::PointerMoved(pos2(
                    400.0 + k as f32 * 40.0,
                    300.0,
                ))]);
            }
            s.push(vec![button(false, 640.0)]);
            s.push(Vec::new());
            script(app, &ctx, start, s);
        };
        mouse(1.0, &mut app);
        assert_eq!(strokes_on_page(&mut app), 0, "pen only by default");
        app.command("input.pen.only", &json!({ "on": false }))
            .unwrap();
        mouse(2.0, &mut app);
        assert_eq!(strokes_on_page(&mut app), 1);
        assert_eq!(app.session.reader_state().unwrap()["page"], 1);
    }

    #[test]
    fn markup_tools_and_colours_come_back_next_launch() {
        let (mut app, _) = markup_app();
        app.command("annotate.tool.highlighter", &json!({}))
            .unwrap();
        app.command("annotate.swatch.select", &json!({ "index": 3 }))
            .unwrap();
        app.command("annotate.style.set", &json!({ "width": 20.0 }))
            .unwrap();
        app.command("annotate.shapes", &json!({ "on": true }))
            .unwrap();
        app.command("input.pen.only", &json!({ "on": false }))
            .unwrap();
        let before = app.session.dispatch("annotate.state", &json!({})).unwrap();
        let saved = Settings::parse(&app.settings.to_json_string());
        let mut next = App::new(Box::new(NoPlatform), saved);
        let after = next.session.dispatch("annotate.state", &json!({})).unwrap();
        for key in [
            "tool",
            "highlighter",
            "highlighterSwatch",
            "penSwatches",
            "shapes",
            "penOnly",
            "pen",
        ] {
            assert_eq!(after[key], before[key], "{key}");
        }
        assert_eq!(after["mode"], false, "markup mode itself is not kept");
    }

    #[test]
    fn libs_10_the_library_window_lists_scores_with_covers() {
        let (mut app, _) = app_with_score(2);
        let ctx = Context::default();
        let r = app.command("ui.library.show", &json!({})).unwrap();
        assert_eq!(r["library"], true);
        let mut ready = 0;
        for k in 0..200 {
            script(&mut app, &ctx, k as f64 * 0.016, vec![Vec::new()]);
            ready = app.library.ready_covers();
            if ready > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(ready, 1, "the score's cover arrives");
        let r = app
            .command("ui.library.view", &json!({ "view": "list" }))
            .unwrap();
        assert_eq!(r["libraryGrid"], false);
        frames(&mut app, &ctx, 1, Vec::new());
        assert_eq!(
            app.command("ui.library.view", &json!({ "view": "shelf" }))
                .unwrap_err()
                .code(),
            "bad_params"
        );
        // Opening a score from the library closes the window.
        let id = app.session.library_score_id().unwrap();
        app.command("score.open", &json!({ "score": id })).unwrap();
        assert!(!app.state.library);
    }

    #[test]
    fn perf_5_imports_report_what_happened() {
        let v = json!({
            "imported": ["a", "b"],
            "skipped": 1,
            "ignored": 3,
            "failed": [{ "path": "/x/broken.pdf", "message": "not a PDF" }],
        });
        assert_eq!(
            import_report(&v),
            "Imported 2 scores; 1 already in the library; 3 files not PDFs; 1 couldn't be read (broken.pdf: not a PDF)."
        );
        assert_eq!(
            import_report(&json!({ "imported": ["a"] })),
            "Imported 1 score."
        );
    }

    #[test]
    fn perf_12_a_list_made_in_the_library_window_plays() {
        let (mut app, _) = app_with_score(2);
        let ctx = Context::default();
        let score = app.session.library_score_id().unwrap();
        let list = app.command("list.new", &json!({ "name": "Gig" })).unwrap()["list"]
            .as_str()
            .unwrap()
            .to_owned();
        app.command("list.entry.add", &json!({ "list": list, "score": score }))
            .unwrap();
        app.command("ui.library.show", &json!({ "on": true }))
            .unwrap();
        app.library.open_list = Some(list.clone());
        frames(&mut app, &ctx, 2, Vec::new());
        app.command("list.play", &json!({ "list": list })).unwrap();
        assert!(!app.state.library, "playing shows the reader");
        frames(&mut app, &ctx, 2, Vec::new());
        let st = app.session.dispatch("list.state", &json!({})).unwrap();
        assert_eq!(
            (st["playing"].as_bool(), st["name"].as_str()),
            (Some(true), Some("Gig"))
        );
    }

    #[test]
    fn the_score_panel_shows_the_open_score_and_its_edits() {
        let (mut app, _) = app_with_score(3);
        let ctx = Context::default();
        app.score_panel.open = true;
        frames(&mut app, &ctx, 2, Vec::new());
        let cached = app.score_cache.clone().unwrap();
        assert_eq!(cached["pages"].as_array().unwrap().len(), 3);
        let id = app.session.library_score_id().unwrap();
        app.command("page.setHidden", &json!({ "score": id, "page": 2 }))
            .unwrap();
        assert!(app.score_cache.is_none(), "an edit drops the stale copy");
        frames(&mut app, &ctx, 1, Vec::new());
        assert_eq!(
            app.score_cache.as_ref().unwrap()["pages"][1]["hidden"],
            true
        );
        assert_eq!(app.session.reader_state().unwrap()["pageCount"], 2);
    }

    #[test]
    fn frames_draw_and_keys_turn() {
        let (mut app, _) = app_with_score(3);
        let ctx = Context::default();
        frames(&mut app, &ctx, 2, Vec::new());
        let st = app.session.reader_state().unwrap();
        assert_eq!(st["page"], 1);
        // The reader area is the window minus the toolbar.
        let vp = st["viewport"]["height"].as_f64().unwrap();
        assert!(vp > 700.0 && vp < 800.0, "{vp}");
        let key = egui::Event::Key {
            key: egui::Key::PageDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frames(&mut app, &ctx, 1, vec![key]);
        assert_eq!(app.session.reader_state().unwrap()["page"], 2);
        // A tap in the right third turns forward.
        let pos = pos2(1200.0, 400.0);
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frames(
            &mut app,
            &ctx,
            1,
            vec![egui::Event::PointerMoved(pos), press(true)],
        );
        frames(&mut app, &ctx, 2, vec![press(false)]);
        assert_eq!(app.session.reader_state().unwrap()["page"], 3);
    }

    #[test]
    fn keep_awake_follows_the_open_score() {
        struct Rec(std::sync::Arc<std::sync::Mutex<Vec<bool>>>);
        impl Platform for Rec {
            fn set_keep_awake(&mut self, on: bool) -> Result<(), String> {
                self.0.lock().unwrap().push(on);
                Ok(())
            }
        }
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = App::new(Box::new(Rec(calls.clone())), Settings::default());
        let ctx = Context::default();
        frames(&mut app, &ctx, 1, Vec::new());
        assert!(
            calls.lock().unwrap().is_empty(),
            "nothing open, nothing held"
        );
        let (_, path) = app_with_score(2);
        app.open_path(&path.to_string_lossy()).unwrap();
        frames(&mut app, &ctx, 1, Vec::new());
        app.command("ui.power.keepAwake", &json!({ "on": false }))
            .unwrap();
        frames(&mut app, &ctx, 1, Vec::new());
        assert_eq!(*calls.lock().unwrap(), vec![true, false]);
    }

    #[test]
    fn a_platform_that_cannot_keep_awake_is_reported_once() {
        struct Never;
        impl Platform for Never {
            fn set_keep_awake(&mut self, _: bool) -> Result<(), String> {
                Err("not supported".into())
            }
        }
        let (_, path) = app_with_score(2);
        let mut app = App::new(Box::new(Never), Settings::default());
        app.open_path(&path.to_string_lossy()).unwrap();
        let ctx = Context::default();
        frames(&mut app, &ctx, 3, Vec::new());
        assert_eq!(app.state.keep_awake.error.as_deref(), Some("not supported"));
        assert!(!app.state.keep_awake.wanted);
    }
}
