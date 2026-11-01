//! scorecraft: The ScoreCraft desktop app (eframe/wgpu) with the TCP control server.
//!
//! `scorecraft [score.pdf]` opens the reader; with no file it reopens the last score. Settings
//! (reader preferences, pedal map, theme, keep-awake) live in the platform's config folder.
//!
//! `--control` starts the control channel (`docs/control-protocol.md`) on a free loopback port
//! and prints `control: 127.0.0.1:<port> token=<hex>`; `--control-port N` picks the port and
//! `--control-file PATH` also writes that line to a file (release Windows builds have no
//! console).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
// Release builds on Windows have no console window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod control;

use std::path::PathBuf;
use std::sync::Arc;

use scorecraft_devices::KeepAwake;
use scorecraft_ui_egui::{App, Platform, Settings};

/// The config folder: `%APPDATA%\ScoreCraft`, `~/Library/Application Support/ScoreCraft` or
/// `$XDG_CONFIG_HOME/scorecraft`.
fn config_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        env("APPDATA").map(|d| d.join("ScoreCraft"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/ScoreCraft"))
    } else {
        env("XDG_CONFIG_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".config")))
            .map(|d| d.join("scorecraft"))
    }
}

/// Where the library lives: beside the settings on Windows and macOS, in `$XDG_DATA_HOME`
/// (`~/.local/share/scorecraft`) on Linux.
fn library_dir() -> Option<PathBuf> {
    if cfg!(any(windows, target_os = "macos")) {
        return config_dir().map(|d| d.join("Library.scorecraft"));
    }
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .map(|d| d.join("scorecraft").join("Library.scorecraft"))
}

fn settings_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("settings.json"))
}

fn load_settings() -> Settings {
    settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| Settings::parse(&t))
        .unwrap_or_default()
}

struct Desktop {
    keep_awake: KeepAwake,
    /// The metronome's click, opened the first time it is wanted.
    click: Option<scorecraft_devices::ClickOut>,
}

impl Platform for Desktop {
    fn pick_score(&mut self) -> Option<String> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            rfd::FileDialog::new()
                .add_filter("PDF scores", &["pdf", "PDF"])
                .pick_file()
                .map(|p| p.to_string_lossy().into_owned())
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            None
        }
    }

    fn set_keep_awake(&mut self, on: bool) -> Result<(), String> {
        self.keep_awake.set(on).map_err(|e| e.to_string())
    }

    fn pick_export_path(&mut self, name: &str) -> Option<String> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name(name)
                .save_file()
                .map(|p| p.to_string_lossy().into_owned())
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = name;
            None
        }
    }

    fn set_click(&mut self, click: Option<(f64, u8)>) -> Result<(), String> {
        if self.click.is_none() {
            if click.is_none() {
                return Ok(());
            }
            self.click = Some(scorecraft_devices::ClickOut::new().map_err(|e| e.to_string())?);
        }
        if let Some(out) = &self.click {
            out.set(click);
        }
        Ok(())
    }

    fn pick_folder(&mut self) -> Option<String> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            rfd::FileDialog::new()
                .pick_folder()
                .map(|p| p.to_string_lossy().into_owned())
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            None
        }
    }

    fn library_dir(&self) -> Option<String> {
        let dir = library_dir()?;
        if let Some(parent) = dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Some(dir.to_string_lossy().into_owned())
    }

    fn save_settings(&mut self, settings: &Settings) {
        let Some(path) = settings_path() else {
            return;
        };
        // Write beside, then rename: a crash mid-write never leaves a half file.
        let tmp = path.with_extension("json.tmp");
        let ok = path
            .parent()
            .is_some_and(|d| std::fs::create_dir_all(d).is_ok())
            && std::fs::write(&tmp, settings.to_json_string()).is_ok();
        if ok {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

struct Shell {
    app: App,
    control: Option<control::Control>,
}

impl eframe::App for Shell {
    fn raw_input_hook(&mut self, ctx: &eframe::egui::Context, raw: &mut eframe::egui::RawInput) {
        if let Some(c) = &mut self.control {
            c.before_frame(&mut self.app, ctx, raw);
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        // Last-resort guard (never-crash rule 7): a panic in a frame is reported and the reader
        // stays on screen.
        let app = &mut self.app;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.ui(ui)));
        if r.is_err() {
            self.app.state.message =
                Some("Something went wrong drawing that frame. Your score is still open.".into());
        }
        if let Some(c) = &mut self.control {
            c.after_frame(&mut self.app, ui.ctx());
        }
    }
}

/// Command-line options.
#[derive(Debug, Default, PartialEq)]
struct Args {
    score: Option<String>,
    control: bool,
    control_port: u16,
    control_file: Option<PathBuf>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--control" => a.control = true,
            "--control-port" => {
                a.control = true;
                a.control_port = args
                    .next()
                    .and_then(|p| p.parse().ok())
                    .ok_or("--control-port wants a port number")?;
            }
            "--control-file" => {
                a.control = true;
                a.control_file = Some(args.next().ok_or("--control-file wants a path")?.into());
            }
            "--help" | "-h" => {
                return Err("usage: scorecraft [--control] [--control-port N] \
                            [--control-file PATH] [score.pdf]"
                    .into());
            }
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ if a.score.is_none() => a.score = Some(arg),
            _ => return Err("give one score at most".into()),
        }
    }
    Ok(a)
}

fn main() -> std::process::ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("scorecraft: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let settings = load_settings();
    let open = args.score.clone().or_else(|| settings.last_score.clone());
    let mut app = App::new(
        Box::new(Desktop {
            keep_awake: KeepAwake::new(),
            click: None,
        }),
        settings,
    );
    if let Some(path) = open
        && let Err(e) = app.open_path(&path)
        && args.score.is_some()
    {
        app.state.message = Some(e.to_string());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("ScoreCraft")
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([480.0, 360.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    match eframe::run_native(
        "ScoreCraft",
        options,
        Box::new(move |cc| {
            let control = if args.control {
                let ctx = cc.egui_ctx.clone();
                let c = control::Control::start(
                    args.control_port,
                    Arc::new(move || ctx.request_repaint()),
                )?;
                let banner = c.banner();
                println!("{banner}");
                if let Some(path) = &args.control_file {
                    std::fs::write(path, format!("{banner}\n"))?;
                }
                Some(c)
            } else {
                None
            };
            Ok(Box::new(Shell { app, control }))
        }),
    ) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("scorecraft: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Result<Args, String> {
        parse_args(s.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn arguments_parse() {
        assert_eq!(args(&[]).unwrap(), Args::default());
        let a = args(&["--control-port", "7000", "x.pdf"]).unwrap();
        assert!(a.control && a.control_port == 7000 && a.score.as_deref() == Some("x.pdf"));
        for bad in [
            &["--control-port"][..],
            &["--nope"],
            &["a.pdf", "b.pdf"],
            &["--control-port", "x"],
        ] {
            assert!(args(bad).is_err(), "{bad:?}");
        }
    }
}
