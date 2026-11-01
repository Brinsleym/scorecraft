//! The shell's own state (AGENTS.md rule 5): serde, so the control channel can read and set it
//! (`ui.state`, `ui.set`). Everything about the score itself lives in the engine.

use serde::{Deserialize, Serialize};

/// App theme (PLAT-4), independent of the page filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub fn id(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn from_id(id: &str) -> Option<Theme> {
        match id {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        }
    }
}

/// Keep-awake (READ-9): what the user wants, and whether the platform is doing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeepAwake {
    /// Keep the screen awake while a score is open.
    pub wanted: bool,
    /// The platform holds the screen awake now.
    pub active: bool,
    /// Why it couldn't, if it couldn't.
    pub error: Option<String>,
}

impl Default for KeepAwake {
    fn default() -> Self {
        KeepAwake {
            wanted: true,
            active: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    /// Toolbar shown. Full screen hides it; a tap in the middle third shows it again.
    pub chrome: bool,
    /// Performance mode (READ-4): the window fills the screen, controls hidden.
    pub full_screen: bool,
    pub keep_awake: KeepAwake,
    pub theme: Theme,
    /// The last error, shown until the next action.
    pub message: Option<String>,
    /// The library window is shown over the reader (LIB-14).
    pub library: bool,
    /// The library shows covers (true) or a list.
    pub library_grid: bool,
}

impl UiState {
    pub fn new() -> UiState {
        UiState {
            chrome: true,
            library_grid: true,
            ..UiState::default()
        }
    }
}
