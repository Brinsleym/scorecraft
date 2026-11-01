//! The browser platform: canvas start-up, the file picker, dropped files and local storage.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::wasm_bindgen::JsCast;
use eframe::wasm_bindgen::closure::Closure;
use scorecraft_ui_egui::{App, Platform, Settings};

/// Files read by the browser, waiting for the next frame: name and bytes.
type Inbox = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

const SETTINGS_KEY: &str = "scorecraft.settings";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Reads `file` and leaves it in `inbox`.
fn read_into(file: web_sys::File, inbox: Inbox, ctx: eframe::egui::Context) {
    wasm_bindgen_futures::spawn_local(async move {
        let name = file.name();
        let Ok(buf) = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await else {
            return;
        };
        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
        inbox.borrow_mut().push((name, bytes));
        ctx.request_repaint();
    });
}

struct Web {
    /// The library's files, kept in memory and mirrored into IndexedDB; handed to the app once.
    library: Option<(scorecraft_format::MemStorage, bool)>,
    inbox: Inbox,
    ctx: Option<eframe::egui::Context>,
    /// The screen wake lock while held (READ-9), released when keep-awake goes off.
    wake: Rc<RefCell<Option<eframe::wasm_bindgen::JsValue>>>,
}

impl Platform for Web {
    fn library_storage(&mut self) -> Option<(scorecraft_engine::AnyStorage, bool)> {
        self.library
            .take()
            .map(|(mem, exists)| (Box::new(mem) as scorecraft_engine::AnyStorage, exists))
    }

    /// Opens the browser's file picker; the file arrives in a later frame.
    fn pick_score(&mut self) -> Option<String> {
        let document = web_sys::window()?.document()?;
        let input: web_sys::HtmlInputElement =
            document.create_element("input").ok()?.dyn_into().ok()?;
        input.set_type("file");
        input.set_accept(".pdf,application/pdf");
        let (inbox, ctx) = (self.inbox.clone(), self.ctx.clone()?);
        let picked = input.clone();
        let on_change = Closure::<dyn FnMut()>::new(move || {
            if let Some(file) = picked.files().and_then(|f| f.get(0)) {
                read_into(file, inbox.clone(), ctx.clone());
            }
        });
        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        // The closure lives as long as the page: one per pick.
        on_change.forget();
        input.click();
        None
    }

    /// The Screen Wake Lock API (`navigator.wakeLock.request("screen")`), where the browser has
    /// it; called by name, so no unstable web-sys features are needed.
    fn set_keep_awake(&mut self, on: bool) -> Result<(), String> {
        use eframe::wasm_bindgen::JsValue;
        use js_sys::{Function, Reflect};
        if !on {
            if let Some(sentinel) = self.wake.borrow_mut().take()
                && let Ok(release) = Reflect::get(&sentinel, &JsValue::from_str("release"))
                && let Ok(release) = release.dyn_into::<Function>()
            {
                let _ = release.call0(&sentinel);
            }
            return Ok(());
        }
        let navigator = web_sys::window()
            .map(|w| JsValue::from(w.navigator()))
            .ok_or_else(|| "no browser window".to_owned())?;
        let lock = Reflect::get(&navigator, &JsValue::from_str("wakeLock"))
            .ok()
            .filter(|v| !v.is_undefined())
            .ok_or_else(|| "this browser can't keep the screen awake".to_owned())?;
        let request: Function = Reflect::get(&lock, &JsValue::from_str("request"))
            .ok()
            .and_then(|f| f.dyn_into().ok())
            .ok_or_else(|| "this browser can't keep the screen awake".to_owned())?;
        let promise: js_sys::Promise = request
            .call1(&lock, &JsValue::from_str("screen"))
            .ok()
            .and_then(|p| p.dyn_into().ok())
            .ok_or_else(|| "the browser refused to keep the screen awake".to_owned())?;
        let held = self.wake.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(sentinel) = wasm_bindgen_futures::JsFuture::from(promise).await {
                *held.borrow_mut() = Some(sentinel);
            }
        });
        Ok(())
    }

    /// Downloads `bytes` as `name` through a temporary object URL.
    fn save_bytes(&mut self, name: &str, bytes: &[u8]) -> bool {
        let save = || -> Option<()> {
            let document = web_sys::window()?.document()?;
            let parts = js_sys::Array::new();
            parts.push(&js_sys::Uint8Array::from(bytes));
            let options = web_sys::BlobPropertyBag::new();
            options.set_type("application/pdf");
            let blob =
                web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).ok()?;
            let url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;
            let a: web_sys::HtmlAnchorElement =
                document.create_element("a").ok()?.dyn_into().ok()?;
            a.set_href(&url);
            a.set_download(name);
            a.click();
            let _ = web_sys::Url::revoke_object_url(&url);
            Some(())
        };
        save().is_some()
    }

    fn save_settings(&mut self, settings: &Settings) {
        if let Some(s) = storage() {
            let _ = s.set_item(SETTINGS_KEY, &settings.to_json_string());
        }
    }
}

struct Shell {
    app: App,
    inbox: Inbox,
    /// The library's memory storage, whose changes are written behind into IndexedDB.
    files: scorecraft_format::MemStorage,
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Dropped files: the browser reads them asynchronously.
        let dropped: Vec<_> = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            let (inbox, ctx2) = (self.inbox.clone(), ctx.clone());
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(bytes) = f.bytes_async().await {
                    let name = f
                        .path()
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "score.pdf".into());
                    inbox.borrow_mut().push((name, bytes));
                    ctx2.request_repaint();
                }
            });
        }
        let ready: Vec<_> = self.inbox.borrow_mut().drain(..).collect();
        for (name, bytes) in ready {
            if let Err(e) = self.app.open_bytes(&name, bytes) {
                self.app.state.message = Some(e.to_string());
            }
        }
        self.app.ui(ui);
        let changes = self.files.take_changes();
        if !changes.is_empty() {
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = crate::idb::apply(changes).await {
                    web_sys::console::error_1(&e);
                }
            });
        }
    }
}

pub fn start() {
    let settings = storage()
        .and_then(|s| s.get_item(SETTINGS_KEY).ok().flatten())
        .map(|t| Settings::parse(&t))
        .unwrap_or_default();
    wasm_bindgen_futures::spawn_local(async move {
        // The library kept in this browser, if any.
        let files = scorecraft_format::MemStorage::new();
        let stored = crate::idb::load().await.unwrap_or_default();
        let exists = stored.iter().any(|(p, _)| p == "manifest.json");
        for (path, data) in &stored {
            files.plant(path, data);
        }
        files.track_changes();
        let Some(canvas) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("scorecraft_canvas"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        else {
            return;
        };
        let inbox: Inbox = Rc::default();
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    let platform = Web {
                        library: Some((files.clone(), exists)),
                        inbox: inbox.clone(),
                        ctx: Some(cc.egui_ctx.clone()),
                        wake: Rc::default(),
                    };
                    let app = App::new(Box::new(platform), settings);
                    Ok(Box::new(Shell { app, inbox, files }))
                }),
            )
            .await;
        if let Err(e) = started {
            web_sys::console::error_1(&e);
        }
    });
}
