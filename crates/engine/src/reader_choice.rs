//! Layout and turn mode: one default for every score, and a choice per score that overrides it
//! (owner, 2026-11-01; spec `docs/spec/page-turning.md`, PT-14). The default comes from
//! settings (`view.defaults.set`). Changing the layout or turn mode in a library score's view
//! remembers it for that score in the library's settings, which sync, outside the undo history.

use serde_json::{Value, json};

use scorecraft_layout::{Layout, TurnMode};

use crate::commands::{CommandSpec, bad, opt_f64, opt_str, params_object, reject_unknown_keys};
use crate::reader::ReaderState;
use crate::score_cmds::score_open;
use crate::{EngineError, Session};

/// Library settings key holding a score's own choice: `reader.<score id>`.
const KEY: &str = "reader.";

/// The layout asked for: one page, two, or automatic (one or two from the reader area's shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutChoice {
    Auto,
    Single,
    TwoPage,
}

impl LayoutChoice {
    pub fn id(self) -> &'static str {
        match self {
            LayoutChoice::Auto => "auto",
            LayoutChoice::Single => "single",
            LayoutChoice::TwoPage => "twoPage",
        }
    }

    pub fn from_id(id: &str) -> Option<LayoutChoice> {
        match id {
            "auto" => Some(LayoutChoice::Auto),
            "single" => Some(LayoutChoice::Single),
            "twoPage" => Some(LayoutChoice::TwoPage),
            _ => None,
        }
    }
}

/// A layout, turn mode and half-page split.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReaderChoice {
    pub layout: LayoutChoice,
    pub mode: TurnMode,
    pub split: f64,
}

impl Default for ReaderChoice {
    /// The automatic layout, a whole page per turn (owner, 2026-11-01).
    fn default() -> Self {
        ReaderChoice {
            layout: LayoutChoice::Auto,
            mode: TurnMode::Full,
            split: 0.5,
        }
    }
}

impl ReaderChoice {
    pub(crate) fn of(r: &ReaderState) -> ReaderChoice {
        ReaderChoice {
            layout: if r.auto_layout {
                LayoutChoice::Auto
            } else {
                match r.reader.layout() {
                    Layout::Single => LayoutChoice::Single,
                    Layout::TwoPage => LayoutChoice::TwoPage,
                }
            },
            mode: r.reader.mode(),
            split: r.reader.split(),
        }
    }

    pub(crate) fn apply(self, r: &mut ReaderState) {
        r.auto_layout = self.layout == LayoutChoice::Auto;
        match self.layout {
            LayoutChoice::Auto => r.resolve_layout(),
            LayoutChoice::Single => r.reader.set_layout(Layout::Single),
            LayoutChoice::TwoPage => r.reader.set_layout(Layout::TwoPage),
        }
        r.reader.set_split(self.split);
        r.reader.set_mode(self.mode);
    }

    pub fn to_json(self) -> Value {
        json!({
            "layout": self.layout.id(),
            "mode": self.mode.id(),
            "split": self.split,
        })
    }

    /// Reads one written by [`Self::to_json`]; anything it can't read keeps `self`'s value. An
    /// older `firstAlone` is ignored: the feature is gone.
    fn read(self, v: &Value) -> ReaderChoice {
        let layout = v
            .get("layout")
            .and_then(Value::as_str)
            .and_then(LayoutChoice::from_id)
            .unwrap_or(self.layout);
        let mode = v
            .get("mode")
            .and_then(Value::as_str)
            .and_then(mode_from_id)
            .unwrap_or(self.mode);
        let split = v
            .get("split")
            .and_then(Value::as_f64)
            .filter(|s| {
                let (lo, hi) = scorecraft_layout::SPLIT_RANGE;
                (lo..=hi).contains(s)
            })
            .unwrap_or(self.split);
        ReaderChoice {
            layout,
            mode,
            split,
        }
    }
}

fn mode_from_id(id: &str) -> Option<TurnMode> {
    match id {
        "full" => Some(TurnMode::Full),
        "halfPage" => Some(TurnMode::HalfPage),
        "alternating" => Some(TurnMode::Alternating),
        _ => None,
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "view.defaults.set",
            label: "Default Layout and Turns",
            menu: "",
            shortcut: None,
            params: r#"{"layout":"auto|single|twoPage"?,"mode":"full|halfPage|alternating"?,"split":number?}"#,
            ours: true,
            enabled: crate::commands::always,
            run: defaults_set as _,
        },
        CommandSpec {
            id: "view.override.clear",
            label: "Use Default Layout for This Score",
            menu: "View>Turn",
            shortcut: None,
            params: "{}",
            ours: true,
            enabled: score_open,
            run: override_clear as _,
        },
    ]
}

/// Sets the default for every score without a choice of its own, and applies it to the open
/// score if that has none. With no params, says what the default is.
fn defaults_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.defaults.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["layout", "mode", "split"])?;
    let mut d = session.reader_default;
    if let Some(l) = opt_str(ID, p, "layout")? {
        d.layout = LayoutChoice::from_id(l)
            .ok_or_else(|| bad(ID, "`layout` must be auto, single or twoPage".into()))?;
    }
    if let Some(m) = opt_str(ID, p, "mode")? {
        d.mode = mode_from_id(m)
            .ok_or_else(|| bad(ID, "`mode` must be full, halfPage or alternating".into()))?;
    }
    if let Some(split) = opt_f64(ID, p, "split")? {
        let (lo, hi) = scorecraft_layout::SPLIT_RANGE;
        if !(lo..=hi).contains(&split) {
            return Err(bad(ID, format!("`split` must be from {lo} to {hi}")));
        }
        d.split = split;
    }
    session.reader_default = d;
    if session.score.is_some() && session.own_choice().is_none() {
        if let Some(open) = session.score.as_mut() {
            d.apply(&mut open.reader);
        }
        session.refresh_reader();
    }
    Ok(json!({ "defaults": d.to_json() }))
}

/// Forgets the open score's own layout and turn mode; it follows the default again.
fn override_clear(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.override.clear";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    session.remember_choice(None)?;
    let d = session.reader_default;
    if let Some(open) = session.score.as_mut() {
        d.apply(&mut open.reader);
    }
    session
        .reader_state()
        .ok_or_else(|| EngineError::Disabled(ID.into()))
}

impl Session {
    /// The open library score's own choice, if it has one.
    pub(crate) fn own_choice(&self) -> Option<ReaderChoice> {
        let id = self.score.as_ref()?.library_score?;
        let raw = self.library()?.settings.get(&format!("{KEY}{id}"))?;
        let v: Value = serde_json::from_str(raw).ok()?;
        Some(self.reader_default.read(&v))
    }

    /// Puts the open score in its own choice, or the default (after opening it).
    pub(crate) fn apply_reader_choice(&mut self) {
        let c = self.own_choice().unwrap_or(self.reader_default);
        if let Some(open) = self.score.as_mut() {
            c.apply(&mut open.reader);
        }
    }

    /// Remembers `choice` as the open library score's own (or forgets it with `None`). Not an
    /// undo step; nothing to do for a score outside the library.
    pub(crate) fn remember_choice(
        &mut self,
        choice: Option<ReaderChoice>,
    ) -> Result<(), EngineError> {
        let Some(id) = self.score.as_ref().and_then(|o| o.library_score) else {
            return Ok(());
        };
        let key = format!("{KEY}{id}");
        let l = self.lib_mut("view")?;
        let value = choice.map(|c| c.to_json().to_string());
        if l.lib.settings.get(&key) == value.as_ref() {
            return Ok(());
        }
        match value {
            Some(v) => l.lib.settings.insert(key, v),
            None => l.lib.settings.remove(&key),
        };
        l.save_untracked(scorecraft_ops::Touched::Library)
    }

    /// After a layout or turn-mode command in the score's view: remember it for this score,
    /// unless `remember` is false (a list carrying the last score's layout over).
    pub(crate) fn remember_view_choice(&mut self, remember: bool) -> Result<(), EngineError> {
        if !remember {
            return Ok(());
        }
        let Some(open) = self.score.as_ref() else {
            return Ok(());
        };
        let c = ReaderChoice::of(&open.reader);
        self.remember_choice(Some(c))
    }
}

#[cfg(test)]
mod tests;
