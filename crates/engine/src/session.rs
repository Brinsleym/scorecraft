use serde_json::Value;

use crate::EngineError;
use crate::commands::{CommandInfo, CommandSpec, registry};

/// One engine session: the registry, the open library with its undo history, and the open
/// score; jobs and preferences as their milestones land (`docs/architecture.md` §4.1, §11).
pub struct Session {
    commands: Vec<CommandSpec>,
    /// The open score, if any. One at a time until the library lands (M5).
    pub(crate) score: Option<crate::score_cmds::OpenScore>,
    /// The open library, if any (M1): model, store, undo history.
    pub(crate) library: Option<crate::library::LibrarySession>,
    pub(crate) clock: crate::library::Clock,
    /// Which key or pedal does what (TURN-4).
    pub(crate) pedal_map: scorecraft_input::PedalMap,
    /// One turn per physical press.
    pub(crate) key_gate: scorecraft_input::KeyGate,
    /// Markup tools, colours, widths and *Draw with Touch* (ANNO-1..7, ANNO-14).
    pub(crate) markup: crate::markup::Markup,
    /// How the library lists scores (LIB-9).
    pub(crate) library_sort: crate::find_cmds::LibrarySort,
    /// The setlist being played, if any (LIST-4).
    pub(crate) playback: Option<crate::play_cmds::Playback>,
    /// The metronome (PRAC-1).
    pub(crate) metronome: scorecraft_practice::Metronome,
    /// Full-text indexes of the library's files, loaded or built by search (LIB-6).
    pub(crate) text_index: crate::fulltext_cmds::TextIndexes,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Session {
            commands: registry(),
            score: None,
            library: None,
            clock: crate::library::default_clock(),
            pedal_map: scorecraft_input::PedalMap::default(),
            key_gate: scorecraft_input::KeyGate::new(scorecraft_input::DEFAULT_DEBOUNCE_MS),
            markup: crate::markup::Markup::default(),
            library_sort: crate::find_cmds::LibrarySort::default(),
            playback: None,
            metronome: scorecraft_practice::Metronome::new(),
            text_index: crate::fulltext_cmds::TextIndexes::new(),
        }
    }

    fn spec(&self, id: &str) -> Option<CommandSpec> {
        self.commands.iter().find(|c| c.id == id).copied()
    }

    /// Runs a command by id. Unknown ids, disabled commands and bad params are errors, never
    /// panics (`panic_hunt` checks this for every command).
    pub fn dispatch(&mut self, id: &str, params: &Value) -> Result<Value, EngineError> {
        let spec = self
            .spec(id)
            .ok_or_else(|| EngineError::UnknownCommand(id.chars().take(128).collect()))?;
        if !(spec.enabled)(self) {
            return Err(EngineError::Disabled(spec.id.to_owned()));
        }
        (spec.run)(self, params)
    }

    /// Whether a registered command is enabled now; `None` if no such command.
    pub fn is_enabled(&self, id: &str) -> Option<bool> {
        self.spec(id).map(|s| (s.enabled)(self))
    }

    /// Every command with its enabled state, in registration order.
    pub fn commands(&self) -> Vec<CommandInfo> {
        self.commands
            .iter()
            .map(|c| CommandInfo {
                id: c.id,
                label: c.label,
                menu: c.menu,
                shortcut: c.shortcut,
                params: c.params,
                ours: c.ours,
                enabled: (c.enabled)(self),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_command_is_an_error_with_a_bounded_message() {
        let mut s = Session::new();
        let id = "x".repeat(10_000);
        let err = s.dispatch(&id, &Value::Null).unwrap_err();
        assert_eq!(err.code(), "unknown_command");
        assert!(err.to_string().len() < 200);
    }

    #[test]
    fn is_enabled_knows_registered_commands_only() {
        let s = Session::new();
        assert_eq!(s.is_enabled("engine.version"), Some(true));
        assert_eq!(s.is_enabled("view.no.such.command"), None);
    }
}
