//! What the shell keeps between launches: reader preferences, the pedal map, theme and
//! keep-awake. The app stores it as JSON in the platform's config folder; the web build in
//! local storage.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use scorecraft_engine::Session;

use crate::state::{Theme, UiState};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub layout: String,
    pub first_alone: bool,
    pub mode: String,
    pub split: f64,
    pub fit_width: bool,
    /// `input.pedal.map`'s `map`: `{"PageDown":"next"}`.
    pub pedal_map: Value,
    pub theme: Theme,
    pub keep_awake: bool,
    /// The score opened last, reopened at launch.
    pub last_score: Option<String>,
    /// Markup tools, colours, swatches, widths and presets: `annotate.state`'s lasting part
    /// (spec `docs/spec/ink.md`: markup opens with the last tool, colours, widths and swatches).
    pub markup: Value,
    /// The *Show ruler button* setting (ANNO-17), off by default.
    pub show_ruler_button: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            layout: "single".into(),
            first_alone: false,
            mode: "full".into(),
            split: 0.5,
            fit_width: false,
            pedal_map: Value::Null,
            theme: Theme::Light,
            keep_awake: true,
            last_score: None,
            markup: Value::Null,
            show_ruler_button: false,
        }
    }
}

/// Largest settings file read.
pub const MAX_SETTINGS_BYTES: usize = 1 << 20;

impl Settings {
    /// Parses a settings file; anything unreadable gives the defaults (a bad file never stops
    /// the app from starting).
    pub fn parse(text: &str) -> Settings {
        if text.len() > MAX_SETTINGS_BYTES {
            return Settings::default();
        }
        serde_json::from_str(text).unwrap_or_default()
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Takes the reader preferences from the open score's reader, if any, and the rest from the
    /// session and shell.
    pub fn capture(&mut self, session: &mut Session, ui: &UiState) {
        if let Some(st) = session.reader_state() {
            let s = |k: &str| st.get(k).and_then(Value::as_str).map(str::to_owned);
            if let Some(v) = s("layout") {
                self.layout = v;
            }
            if let Some(v) = s("mode") {
                self.mode = v;
            }
            self.first_alone = st
                .get("firstAlone")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.split = st.get("split").and_then(Value::as_f64).unwrap_or(0.5);
            self.fit_width = st.pointer("/zoom/fit").and_then(Value::as_str) == Some("width");
        }
        if let Ok(m) = session.dispatch("input.pedal.map", &json!({})) {
            self.pedal_map = m.get("map").cloned().unwrap_or(Value::Null);
        }
        self.theme = ui.theme;
        self.keep_awake = ui.keep_awake.wanted;
        if let Ok(mut m) = session.dispatch("annotate.state", &json!({})) {
            if let Some(o) = m.as_object_mut() {
                // What belongs to this moment or this score, or follows from the rest.
                for k in [
                    "mode",
                    "ruler",
                    "selection",
                    "layer",
                    "swatches",
                    "swatch",
                    "palette",
                    "penPalette",
                    "highlighterPalette",
                    "touchLabel",
                    "drawWithTouch",
                ] {
                    o.remove(k);
                }
            }
            self.markup = m;
        }
    }

    /// Restores the markup tools saved by [`Settings::capture`]. Each step is best effort: a
    /// value the engine refuses keeps its default.
    pub fn apply_markup(&self, session: &mut Session) {
        let m = &self.markup;
        if !m.is_object() {
            return;
        }
        let mut run = |id: &str, p: Value| {
            let _ = session.dispatch(id, &p);
        };
        let get = |path: &str| m.pointer(path).cloned().unwrap_or(Value::Null);
        let tool_cmd = |tool: &str| match tool {
            "highlighter" => "annotate.tool.highlighter",
            "whiteout" => "annotate.tool.whiteout",
            _ => "annotate.tool.pen",
        };
        for p in m
            .get("presets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let tool = p.get("tool").and_then(Value::as_str).unwrap_or("pen");
            let mut style = json!({ "width": p.get("width") });
            if tool != "whiteout"
                && let Some(o) = style.as_object_mut()
            {
                o.insert(
                    "color".into(),
                    p.get("color").cloned().unwrap_or(Value::Null),
                );
            }
            run(tool_cmd(tool), style);
            run("annotate.pen.preset.set", json!({ "name": p.get("name") }));
        }
        for (tool, list, chosen) in [
            ("annotate.tool.pen", "/penSwatches", "/penSwatch"),
            (
                "annotate.tool.highlighter",
                "/highlighterSwatches",
                "/highlighterSwatch",
            ),
        ] {
            run(tool, json!({}));
            for (i, c) in get(list).as_array().into_iter().flatten().enumerate() {
                run("annotate.swatch.set", json!({ "color": c, "index": i + 1 }));
            }
            if get(chosen).is_u64() {
                run("annotate.swatch.select", json!({ "index": get(chosen) }));
            }
        }
        run(
            "annotate.tool.highlighter",
            json!({ "color": get("/highlighter/color"), "width": get("/highlighter/width") }),
        );
        run(
            "annotate.tool.whiteout",
            json!({ "width": get("/whiteout/width") }),
        );
        let eraser = if get("/eraser/mode") == "pixel" {
            "annotate.tool.pixelEraser"
        } else {
            "annotate.tool.strokeEraser"
        };
        run(eraser, json!({ "width": get("/eraser/width") }));
        run("annotate.tool.text", json!({}));
        run(
            "annotate.style.set",
            json!({ "width": get("/text/size"), "color": get("/text/color") }),
        );
        run(
            "annotate.tool.stamp",
            json!({ "stamp": get("/stamp/stamp"), "size": get("/stamp/size") }),
        );
        run(
            "annotate.tool.pen",
            json!({ "color": get("/pen/color"), "width": get("/pen/width") }),
        );
        for (id, key) in [
            ("input.pen.only", "/penOnly"),
            ("annotate.highlighter.roundEnds", "/highlighterRoundEnds"),
            ("annotate.shapes", "/shapes"),
        ] {
            if get(key).is_boolean() {
                run(id, json!({ "on": get(key) }));
            }
        }
        if get("/penButton").is_string() {
            run(
                "annotate.pen.buttonTool",
                json!({ "tool": get("/penButton") }),
            );
        }
        if get("/touchLock") == true {
            run("annotate.mode.set", json!({ "on": true }));
            run("input.touch.lock", json!({ "on": true }));
            run("annotate.mode.set", json!({ "on": false }));
        }
        let last = match get("/tool").as_str() {
            Some("pen") if get("/whiteoutOn") == true => "annotate.tool.whiteout",
            Some("highlighter") => "annotate.tool.highlighter",
            Some("eraser") => eraser,
            Some("text") => "annotate.tool.text",
            Some("stamp") => "annotate.tool.stamp",
            Some("select") => "annotate.tool.select",
            _ => "annotate.tool.pen",
        };
        run(last, json!({}));
    }

    /// Applies the pedal map. Bad entries are ignored, keeping the default map.
    pub fn apply_to_session(&self, session: &mut Session) {
        if self.pedal_map.is_object() {
            let _ = session.dispatch("input.pedal.map", &json!({ "map": self.pedal_map }));
        }
    }

    /// Applies the reader preferences to a score just opened. Each is best effort.
    pub fn apply_to_reader(&self, session: &mut Session) {
        let _ = if self.layout == "twoPage" {
            session.dispatch(
                "view.layout.twoPage",
                &json!({ "firstAlone": self.first_alone }),
            )
        } else {
            session.dispatch("view.layout.single", &json!({}))
        };
        let _ = match self.mode.as_str() {
            "halfPage" => session.dispatch("view.turn.halfPage", &json!({ "split": self.split })),
            "alternating" => session.dispatch("view.turn.alternating", &json!({})),
            _ => Ok(Value::Null),
        };
        if self.fit_width {
            let _ = session.dispatch("view.zoom.landscapeFit", &json!({}));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_files_give_defaults_and_good_ones_round_trip() {
        assert_eq!(Settings::parse("not json"), Settings::default());
        assert_eq!(Settings::parse(r#"{"layout": 5}"#), Settings::default());
        let s = Settings {
            layout: "twoPage".into(),
            theme: Theme::Dark,
            last_score: Some("/scores/a.pdf".into()),
            ..Settings::default()
        };
        assert_eq!(Settings::parse(&s.to_json_string()), s);
        // Unknown and missing fields are fine.
        let p = Settings::parse(r#"{"mode":"halfPage","future":1}"#);
        assert_eq!((p.mode.as_str(), p.layout.as_str()), ("halfPage", "single"));
    }
}
