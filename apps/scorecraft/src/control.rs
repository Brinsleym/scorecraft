//! Serves the control channel from the UI thread. Requests are drained before each frame:
//! commands and state reads are answered at once; `ui.key` and `ui.pointer` inject their events
//! into this frame and answer with the state after it; `ui.screenshot` answers when the
//! integration hands back the next frame's pixels.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{Context, RawInput, ViewportCommand};
use scorecraft_automation::control::{self, Reply, Request, Server};
use scorecraft_engine::EngineError;
use scorecraft_ui_egui::App;
use scorecraft_ui_egui::control::{input_events, screenshot_json};
use serde_json::Value;

pub struct Control {
    server: Server,
    /// Requests answered with the state once the current frame has run.
    after_frame: Vec<Request>,
    /// Screenshots waiting for the visible pages to finish rendering, with when they arrived.
    shots: Vec<(Request, Instant)>,
}

/// Longest a screenshot waits for pages still rendering before it captures what is shown.
const SHOT_WAIT: Duration = Duration::from_secs(15);

fn engine_err(e: EngineError) -> (String, String) {
    (e.code().to_owned(), e.to_string())
}

impl Control {
    /// Starts the server on `port` (0 picks one). `wake` repaints the window.
    pub fn start(port: u16, wake: Arc<dyn Fn() + Send + Sync>) -> Result<Control, String> {
        let token = control::new_token().ok_or("the OS gave no randomness for a token")?;
        let server = control::start(port, token, wake).map_err(|e| e.to_string())?;
        Ok(Control {
            server,
            after_frame: Vec::new(),
            shots: Vec::new(),
        })
    }

    /// The line printed at startup for agents to read.
    pub fn banner(&self) -> String {
        format!("control: {} token={}", self.server.addr, self.server.token)
    }

    /// Before the frame (eframe's raw input hook).
    pub fn before_frame(&mut self, app: &mut App, ctx: &Context, raw: &mut RawInput) {
        while let Ok(req) = self.server.requests.try_recv() {
            if self.handle(app, ctx, raw, req) {
                // Input waits for its frame; later requests see its effect next frame.
                ctx.request_repaint();
                break;
            }
        }
    }

    /// After the frame: answer the input requests with the new state, and take the screenshots
    /// whose pages are ready.
    pub fn after_frame(&mut self, app: &mut App, ctx: &Context) {
        for req in self.after_frame.drain(..) {
            req.ok(app.control_state());
        }
        if self.shots.is_empty() {
            return;
        }
        let ready = app.pages_ready();
        let (take, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.shots)
            .into_iter()
            .partition(|(_, since)| ready || since.elapsed() >= SHOT_WAIT);
        self.shots = wait;
        for (req, _) in take {
            // Paint even if the window is minimised or covered.
            ctx.send_viewport_cmd(ViewportCommand::RequestPaintWhileHidden);
            ctx.request_screenshot(move |image| match screenshot_json(&image) {
                Ok(v) => req.ok(v),
                Err(e) => req.err("io", e),
            });
        }
        if !self.shots.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }

    /// Answers or parks one request; true if it injected input into this frame.
    fn handle(&mut self, app: &mut App, ctx: &Context, raw: &mut RawInput, req: Request) -> bool {
        let list = app.control_commands();
        let mut run = |id: &str, p: &Value| -> Reply { app.command(id, p).map_err(engine_err) };
        if let Some(r) =
            control::handle_common(&req.method, &req.params, &mut run, &|| list.clone())
        {
            answer(req, r);
            return false;
        }
        match req.method.as_str() {
            "ui.state" => req.ok(app.control_state()),
            "ui.set" => match app.control_set(&req.params) {
                Ok(v) => req.ok(v),
                Err(e) => req.err("bad_params", e),
            },
            "ui.key" | "ui.pointer" => match input_events(&req.method, &req.params) {
                Ok(events) => {
                    raw.events.extend(events);
                    // A minimised window still runs the pass that applies them.
                    ctx.send_viewport_cmd(ViewportCommand::RequestPaintWhileHidden);
                    self.after_frame.push(req);
                    return true;
                }
                Err(e) => req.err("bad_params", e),
            },
            "ui.screenshot" => {
                // Taken after a frame in which every visible page is rendered.
                ctx.send_viewport_cmd(ViewportCommand::RequestPaintWhileHidden);
                self.shots.push((req, Instant::now()));
            }
            _ => req.err(
                "unknown_method",
                "methods: command, command_batch, command_list, ui.state, ui.set, ui.key, \
                 ui.pointer, ui.screenshot",
            ),
        }
        false
    }
}

fn answer(req: Request, r: Reply) {
    match r {
        Ok(v) => req.ok(v),
        Err((code, message)) => req.err(&code, message),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use base64::Engine as _;
    use eframe::egui::{self, Color32, ColorImage, ViewportId, pos2, vec2};
    use scorecraft_automation::control::Client;
    use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};
    use scorecraft_ui_egui::{NoPlatform, Settings};
    use serde_json::json;

    use super::*;

    /// The M3 definition of done: an agent turns the page over the control channel and takes a
    /// screenshot of it. The window runs headless here, so the GPU readback is stood in for by
    /// a solid image of the window's size.
    #[test]
    fn an_agent_turns_a_page_and_screenshots_it() {
        let dir = std::env::temp_dir().join(format!("scorecraft-control-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("score.pdf");
        std::fs::write(
            &path,
            PdfBuilder::new().pages(3, PageSpec::default()).build(),
        )
        .unwrap();
        // One page at a time, so each turn moves one page (the default is automatic).
        let settings = Settings {
            layout: "single".into(),
            ..Settings::default()
        };
        let mut app = App::new(Box::new(NoPlatform), settings);
        app.open_path(&path.to_string_lossy()).unwrap();

        let ctx = Context::default();
        let mut control = Control::start(0, Arc::new(|| {})).unwrap();
        let banner = control.banner();
        let (addr, token) = (control.server.addr, control.server.token.clone());
        assert!(banner.contains(&addr.to_string()) && banner.contains(&token));

        let agent = std::thread::spawn(move || {
            let mut c = Client::connect(addr, &token).unwrap();
            let before = c.call("ui.state", json!({})).unwrap();
            let turned = c.call("ui.key", json!({ "key": "ArrowRight" })).unwrap();
            let by_command = c
                .call("command", json!({ "id": "view.turn.next", "params": {} }))
                .unwrap();
            let shot = c.call("ui.screenshot", json!({})).unwrap();
            let set = c
                .call("ui.set", json!({ "path": "theme", "value": "dark" }))
                .unwrap();
            let listed = c.call("command_list", json!({ "filter": "ui." })).unwrap();
            let bad = c.call("ui.key", json!({ "key": "NoSuchKey" })).unwrap();
            let unknown = c.call("ui.dance", json!({})).unwrap();
            (before, turned, by_command, shot, set, listed, bad, unknown)
        });

        // The UI thread: frames until the agent is done.
        for _ in 0..2000 {
            if agent.is_finished() {
                break;
            }
            let mut raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    pos2(0.0, 0.0),
                    vec2(800.0, 600.0),
                )),
                ..Default::default()
            };
            control.before_frame(&mut app, &ctx, &mut raw);
            let mut out = ctx.run_ui(raw, |ui| app.ui(ui));
            control.after_frame(&mut app, &ctx);
            out.textures_delta.clear();
            if let Some(vp) = out.viewport_output.remove(&ViewportId::ROOT) {
                for cmd in vp.commands {
                    if let egui::ViewportCommand::Screenshot(cb) = cmd {
                        cb.complete(Arc::new(ColorImage::new(
                            [800, 600],
                            vec![Color32::WHITE; 800 * 600],
                        )));
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let (before, turned, by_command, shot, set, listed, bad, unknown) = agent.join().unwrap();
        assert_eq!(before["result"]["reader"]["page"], 1);
        assert_eq!(turned["result"]["reader"]["page"], 2, "{turned}");
        assert_eq!(by_command["result"]["page"], 3, "{by_command}");
        assert_eq!(shot["result"]["width"], 800, "{shot}");
        let png = base64::engine::general_purpose::STANDARD
            .decode(shot["result"]["png"].as_str().unwrap())
            .unwrap();
        assert_eq!(&png[..4], b"\x89PNG");
        assert_eq!(set["result"]["ui"]["theme"], "dark");
        assert!(listed["result"].as_array().unwrap().len() >= 4);
        assert_eq!(bad["error"]["code"], "bad_params");
        assert_eq!(unknown["error"]["code"], "unknown_method");
    }
}
