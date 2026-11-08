//! What the control channel (`docs/control-protocol.md`) needs from the shell: its state, the
//! command list with the shell commands, `ui.set` paths, input events for `ui.key` and
//! `ui.pointer`, and screenshot encoding. The transport lives in `scorecraft-automation`; the
//! app wires the two together.

use base64::Engine as _;
use egui::{ColorImage, Event, Key, Modifiers, PointerButton, pos2};
use serde_json::{Value, json};

use crate::app::{App, SHELL_COMMANDS};

/// `ui.set` paths and the shell command each one runs, with the param it fills.
pub const SET_PATHS: &[(&str, &str, &str)] = &[
    ("theme", "ui.theme.set", "theme"),
    ("fullScreen", "ui.view.fullScreen", "on"),
    ("chrome", "ui.chrome.toggle", "on"),
    ("keepAwake", "ui.power.keepAwake", "on"),
];

/// Largest screenshot encoded, in pixels.
const MAX_SCREENSHOT_PIXELS: usize = 64_000_000;

impl App {
    /// `ui.state`: the shell state and the reader's (or `null` with no score open).
    pub fn control_state(&mut self) -> Value {
        json!({
            "ui": serde_json::to_value(&self.state).unwrap_or(Value::Null),
            "reader": self.session.reader_state(),
        })
    }

    /// `command_list`: every engine command and the shell commands.
    pub fn control_commands(&self) -> Value {
        let mut all: Vec<Value> = self
            .session
            .commands()
            .into_iter()
            .map(|c| c.to_json())
            .collect();
        all.extend(SHELL_COMMANDS.iter().map(|(id, label, params)| {
            json!({
                "id": id, "label": label, "menu": "View", "shortcut": null,
                "params": params, "ours": true, "enabled": true,
            })
        }));
        Value::Array(all)
    }

    /// `ui.set {path, value}`: runs the shell command behind `path`.
    pub fn control_set(&mut self, params: &Value) -> Result<Value, String> {
        let path = params.get("path").and_then(Value::as_str).unwrap_or("");
        let Some((_, id, key)) = SET_PATHS.iter().find(|(p, _, _)| *p == path) else {
            let known: Vec<&str> = SET_PATHS.iter().map(|p| p.0).collect();
            return Err(format!("`path` must be one of {}", known.join(", ")));
        };
        let value = params.get("value").cloned().unwrap_or(Value::Null);
        self.command(id, &json!({ *key: value }))
            .map_err(|e| e.to_string())?;
        Ok(self.control_state())
    }
}

fn modifiers(params: &Value) -> Result<Modifiers, String> {
    let mut m = Modifiers::NONE;
    let list = match params.get("modifiers") {
        None | Some(Value::Null) => return Ok(m),
        Some(Value::Array(a)) => a,
        Some(_) => return Err("`modifiers` must be a list".into()),
    };
    for v in list {
        match v.as_str().map(str::to_ascii_lowercase).as_deref() {
            Some("ctrl" | "control") => {
                m.ctrl = true;
                m.command = true;
            }
            Some("alt" | "option") => m.alt = true,
            Some("shift") => m.shift = true,
            Some("cmd" | "command" | "meta") => {
                m.mac_cmd = cfg!(target_os = "macos");
                m.command = true;
            }
            _ => return Err(format!("unknown modifier {v}; use ctrl, alt, shift or cmd")),
        }
    }
    Ok(m)
}

fn coord(params: &Value, k: &str) -> Result<f32, String> {
    params
        .get(k)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && v.abs() <= 1e6)
        .map(|v| v as f32)
        .ok_or_else(|| format!("`{k}` must be a number of points"))
}

/// The egui events for `ui.key {key, modifiers}` (a press and a release) or
/// `ui.pointer {phase: down|move|up, x, y, button?: primary|secondary}` (window points;
/// `secondary` is a right-click, which opens context menus).
pub fn input_events(method: &str, params: &Value) -> Result<Vec<Event>, String> {
    match method {
        "ui.key" => {
            let name = params
                .get("key")
                .and_then(Value::as_str)
                .ok_or("`key` is required, such as \"ArrowRight\" or \"PageDown\"")?;
            let key = Key::from_name(name).ok_or_else(|| format!("unknown key {name:?}"))?;
            let modifiers = modifiers(params)?;
            Ok([true, false]
                .into_iter()
                .map(|pressed| Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                })
                .collect())
        }
        "ui.pointer" => {
            let pos = pos2(coord(params, "x")?, coord(params, "y")?);
            let modifiers = modifiers(params)?;
            let which = match params.get("button").and_then(Value::as_str) {
                None | Some("primary") => PointerButton::Primary,
                Some("secondary") => PointerButton::Secondary,
                Some(other) => {
                    return Err(format!(
                        "`button` must be primary or secondary, not {other:?}"
                    ));
                }
            };
            let button = |pressed| Event::PointerButton {
                pos,
                button: which,
                pressed,
                modifiers,
            };
            match params.get("phase").and_then(Value::as_str) {
                Some("down") => Ok(vec![Event::PointerMoved(pos), button(true)]),
                Some("move") => Ok(vec![Event::PointerMoved(pos)]),
                Some("up") => Ok(vec![Event::PointerMoved(pos), button(false)]),
                _ => Err("`phase` must be down, move or up".into()),
            }
        }
        _ => Err(format!("{method} is not an input method")),
    }
}

/// `ui.screenshot`'s result: the window's pixels as a base64 PNG.
pub fn screenshot_json(image: &ColorImage) -> Result<Value, String> {
    let [w, h] = image.size;
    if w.saturating_mul(h) > MAX_SCREENSHOT_PIXELS {
        return Err(format!("the window is over {MAX_SCREENSHOT_PIXELS} pixels"));
    }
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    let (w32, h32) = (
        u32::try_from(w).map_err(|e| e.to_string())?,
        u32::try_from(h).map_err(|e| e.to_string())?,
    );
    let png = scorecraft_codecs::png::encode_premultiplied(w32, h32, &rgba)?;
    Ok(json!({
        "width": w,
        "height": h,
        "png": base64::engine::general_purpose::STANDARD.encode(png),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NoPlatform, Settings};

    #[test]
    fn keys_and_pointers_become_events() {
        let e = input_events(
            "ui.key",
            &json!({ "key": "ArrowRight", "modifiers": ["shift"] }),
        )
        .unwrap();
        assert_eq!(e.len(), 2);
        assert!(matches!(
            e[0],
            Event::Key { key: Key::ArrowRight, pressed: true, modifiers, .. } if modifiers.shift
        ));
        let e = input_events("ui.pointer", &json!({ "phase": "down", "x": 10, "y": 20 })).unwrap();
        assert!(matches!(e[1], Event::PointerButton { pressed: true, .. }));
        for (m, p) in [
            ("ui.key", json!({})),
            ("ui.key", json!({ "key": "Nope" })),
            ("ui.key", json!({ "key": "A", "modifiers": ["hyper"] })),
            ("ui.pointer", json!({ "phase": "down", "x": "1", "y": 2 })),
            ("ui.pointer", json!({ "phase": "hover", "x": 1, "y": 2 })),
            (
                "ui.pointer",
                json!({ "phase": "down", "x": 1, "y": 2, "button": "middle" }),
            ),
            ("ui.screenshot", json!({})),
        ] {
            assert!(input_events(m, &p).is_err(), "{m} {p}");
        }
    }

    #[test]
    fn set_paths_run_shell_commands() {
        let mut app = App::new(Box::new(NoPlatform), Settings::default());
        let s = app
            .control_set(&json!({ "path": "theme", "value": "dark" }))
            .unwrap();
        assert_eq!(s["ui"]["theme"], "dark");
        assert!(s["reader"].is_null());
        assert!(
            app.control_set(&json!({ "path": "theme", "value": "pink" }))
                .is_err()
        );
        assert!(
            app.control_set(&json!({ "path": "nope", "value": 1 }))
                .is_err()
        );
        let list = app.control_commands();
        let ids: Vec<&str> = list
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c["id"].as_str())
            .collect();
        assert!(ids.contains(&"view.turn.next") && ids.contains(&"ui.theme.set"));
    }

    #[test]
    fn screenshots_encode_as_png() {
        let img = ColorImage::new([4, 3], vec![egui::Color32::RED; 12]);
        let v = screenshot_json(&img).unwrap();
        assert_eq!(
            (v["width"].as_u64(), v["height"].as_u64()),
            (Some(4), Some(3))
        );
        let png = base64::engine::general_purpose::STANDARD
            .decode(v["png"].as_str().unwrap())
            .unwrap();
        assert_eq!(&png[..4], b"\x89PNG");
    }
}
