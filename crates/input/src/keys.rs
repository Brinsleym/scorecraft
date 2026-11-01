//! Keys and foot pedals (TURN-3, TURN-4). A Bluetooth page-turn pedal is a keyboard to the
//! operating system, so a pedal press arrives as a key; the [`PedalMap`] says what each key does.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// What a key, pedal or tap asks the reader to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Next,
    Previous,
    First,
    Last,
}

impl Action {
    pub const ALL: [Action; 4] = [Action::Next, Action::Previous, Action::First, Action::Last];

    pub fn id(self) -> &'static str {
        match self {
            Action::Next => "next",
            Action::Previous => "previous",
            Action::First => "first",
            Action::Last => "last",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    /// The engine command that performs it, and the `to` param for `view.page.goto`.
    pub fn command(self) -> (&'static str, Option<&'static str>) {
        match self {
            Action::Next => ("view.turn.next", None),
            Action::Previous => ("view.turn.previous", None),
            Action::First => ("view.page.goto", Some("first")),
            Action::Last => ("view.page.goto", Some("last")),
        }
    }
}

/// Longest key name accepted.
const MAX_KEY_NAME: usize = 32;

/// A key with modifiers, written `Shift+Space`, `Ctrl+Alt+P`, `PageDown`. Key names follow the
/// web's `KeyboardEvent.key` values for named keys (`ArrowRight`, `PageDown`, `Enter`), with
/// `Space` for the space bar and upper-case letters; a few common aliases are accepted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: String,
}

impl Chord {
    /// A chord without modifiers. `None` for a name [`Chord::parse`] wouldn't accept.
    pub fn key(name: &str) -> Option<Chord> {
        Some(Chord {
            ctrl: false,
            alt: false,
            shift: false,
            key: canonical_key(name)?,
        })
    }

    /// Parses `Shift+Space`-style text. Modifier names are case-insensitive; `Cmd` and `Meta`
    /// count as Ctrl so one map works on every platform.
    pub fn parse(text: &str) -> Option<Chord> {
        if text.len() > 4 * MAX_KEY_NAME {
            return None;
        }
        let parts: Vec<&str> = text.split('+').collect();
        let (key, mods) = parts.split_last()?;
        let mut chord = Chord::key(key)?;
        for m in mods {
            match m.trim().to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "command" | "meta" => chord.ctrl = true,
                "alt" | "option" => chord.alt = true,
                "shift" => chord.shift = true,
                _ => return None,
            }
        }
        Some(chord)
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        f.write_str(&self.key)
    }
}

/// The canonical spelling of a key name, or `None` if it isn't one.
fn canonical_key(name: &str) -> Option<String> {
    if name == " " {
        return Some("Space".to_owned());
    }
    let name = name.trim();
    if name.is_empty()
        || name.len() > MAX_KEY_NAME
        || !name.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    let alias = match lower.as_str() {
        "right" | "arrowright" => "ArrowRight",
        "left" | "arrowleft" => "ArrowLeft",
        "up" | "arrowup" => "ArrowUp",
        "down" | "arrowdown" => "ArrowDown",
        "pagedown" | "pgdn" | "next" => "PageDown",
        "pageup" | "pgup" | "prior" => "PageUp",
        "space" | "spacebar" => "Space",
        "enter" | "return" => "Enter",
        "home" => "Home",
        "end" => "End",
        "backspace" => "Backspace",
        "tab" => "Tab",
        "escape" | "esc" => "Escape",
        _ => "",
    };
    if !alias.is_empty() {
        return Some(alias.to_owned());
    }
    if lower.len() == 1 {
        return Some(lower.to_ascii_uppercase());
    }
    if let Some(n) = lower.strip_prefix('f')
        && n.parse::<u8>().is_ok_and(|n| (1..=24).contains(&n))
    {
        return Some(format!("F{n}"));
    }
    // Other named keys (media keys and the like): keep the frontend's spelling.
    Some(name.to_owned())
}

/// A named starting point for the pedal map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    pub id: &'static str,
    pub label: &'static str,
    entries: &'static [(&'static str, Action)],
}

impl Profile {
    pub fn by_id(id: &str) -> Option<&'static Profile> {
        PROFILES.iter().find(|p| p.id == id)
    }
}

/// The pedal profiles. `default` takes every common page-turn key, so most keyboard-emulating
/// pedals work without setup; the others suit a pedal whose spare keys the player uses for
/// something else.
pub static PROFILES: [Profile; 4] = [
    Profile {
        id: "default",
        label: "Any page-turn key",
        entries: &[
            ("ArrowRight", Action::Next),
            ("ArrowDown", Action::Next),
            ("PageDown", Action::Next),
            ("Space", Action::Next),
            ("ArrowLeft", Action::Previous),
            ("ArrowUp", Action::Previous),
            ("PageUp", Action::Previous),
            ("Shift+Space", Action::Previous),
            ("Enter", Action::Next),
            ("Backspace", Action::Previous),
            ("Home", Action::First),
            ("End", Action::Last),
        ],
    },
    Profile {
        id: "leftRight",
        label: "Left and right arrows",
        entries: &[
            ("ArrowRight", Action::Next),
            ("ArrowLeft", Action::Previous),
        ],
    },
    Profile {
        id: "upDown",
        label: "Up and down arrows",
        entries: &[("ArrowDown", Action::Next), ("ArrowUp", Action::Previous)],
    },
    Profile {
        id: "pageUpDown",
        label: "Page Up and Page Down",
        entries: &[("PageDown", Action::Next), ("PageUp", Action::Previous)],
    },
];

/// Most keys a map holds.
pub const MAX_ENTRIES: usize = 64;

/// Which key does what. Serialised as a JSON object `{"PageDown":"next","Shift+Space":"previous"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedalMap {
    entries: Vec<(Chord, Action)>,
}

impl Default for PedalMap {
    fn default() -> Self {
        PedalMap::profile(&PROFILES[0])
    }
}

impl PedalMap {
    pub fn profile(p: &Profile) -> PedalMap {
        PedalMap {
            entries: p
                .entries
                .iter()
                .filter_map(|(k, a)| Some((Chord::parse(k)?, *a)))
                .collect(),
        }
    }

    /// No keys mapped.
    pub fn empty() -> PedalMap {
        PedalMap {
            entries: Vec::new(),
        }
    }

    /// What `chord` does, if anything.
    pub fn action(&self, chord: &Chord) -> Option<Action> {
        self.entries
            .iter()
            .find(|(c, _)| c == chord)
            .map(|(_, a)| *a)
    }

    /// Maps `chord` to `action`, or unmaps it with `None`. False (and no change) when the map is
    /// full.
    pub fn set(&mut self, chord: Chord, action: Option<Action>) -> bool {
        let at = self.entries.iter().position(|(c, _)| *c == chord);
        match (at, action) {
            (Some(i), Some(a)) => {
                if let Some(e) = self.entries.get_mut(i) {
                    e.1 = a;
                }
            }
            (Some(i), None) => {
                self.entries.remove(i);
            }
            (None, Some(a)) => {
                if self.entries.len() >= MAX_ENTRIES {
                    return false;
                }
                self.entries.push((chord, a));
            }
            (None, None) => {}
        }
        true
    }

    pub fn entries(&self) -> &[(Chord, Action)] {
        &self.entries
    }

    /// The keys mapped to `action`.
    pub fn keys_for(&self, action: Action) -> Vec<&Chord> {
        self.entries
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(c, _)| c)
            .collect()
    }
}

impl Serialize for PedalMap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.entries.len()))?;
        for (c, a) in &self.entries {
            m.serialize_entry(&c.to_string(), a.id())?;
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for PedalMap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let raw = std::collections::BTreeMap::<String, String>::deserialize(d)?;
        if raw.len() > MAX_ENTRIES {
            return Err(D::Error::custom(format!("more than {MAX_ENTRIES} keys")));
        }
        let mut map = PedalMap::empty();
        for (k, v) in raw {
            let chord = Chord::parse(&k).ok_or_else(|| D::Error::custom("not a key"))?;
            let action = Action::from_id(&v).ok_or_else(|| D::Error::custom("not an action"))?;
            map.set(chord, Some(action));
        }
        Ok(map)
    }
}

/// Lets one press through per physical press: key repeat from a held key or pedal is dropped,
/// and so is a second press of the same key within the debounce time (some pedals bounce).
#[derive(Debug, Clone, Default)]
pub struct KeyGate {
    last: Option<(Chord, u64)>,
    debounce_ms: u64,
}

/// Default debounce for repeated presses of one key.
pub const DEFAULT_DEBOUNCE_MS: u64 = 80;

impl KeyGate {
    pub fn new(debounce_ms: u64) -> KeyGate {
        KeyGate {
            last: None,
            debounce_ms,
        }
    }

    /// Whether a key-down of `chord` at `now_ms` counts. `repeat` is the platform's auto-repeat
    /// flag.
    pub fn press(&mut self, chord: &Chord, repeat: bool, now_ms: u64) -> bool {
        if repeat {
            return false;
        }
        if let Some((c, t)) = &self.last
            && c == chord
            && now_ms >= *t
            && now_ms - t < self.debounce_ms
        {
            return false;
        }
        self.last = Some((chord.clone(), now_ms));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_parse_and_print_canonically() {
        for (text, shown) in [
            ("Right", "ArrowRight"),
            ("pgdn", "PageDown"),
            ("shift+space", "Shift+Space"),
            (" ", "Space"),
            ("ctrl+alt+p", "Ctrl+Alt+P"),
            ("Cmd+G", "Ctrl+G"),
            ("f5", "F5"),
            ("AudioVolumeUp", "AudioVolumeUp"),
        ] {
            assert_eq!(Chord::parse(text).unwrap().to_string(), shown, "{text}");
        }
        for bad in ["", "+", "Hyper+A", "Ctrl+", "a b", "é", &"x".repeat(200)] {
            assert!(Chord::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn default_profile_takes_common_pedal_keys() {
        let m = PedalMap::default();
        let act = |k: &str| m.action(&Chord::parse(k).unwrap());
        assert_eq!(act("PageDown"), Some(Action::Next));
        assert_eq!(act("Space"), Some(Action::Next));
        assert_eq!(act("Shift+Space"), Some(Action::Previous));
        assert_eq!(act("ArrowLeft"), Some(Action::Previous));
        assert_eq!(act("End"), Some(Action::Last));
        assert_eq!(act("Q"), None);
        for p in &PROFILES {
            assert_eq!(
                PedalMap::profile(p).entries().len(),
                p.entries.len(),
                "{}",
                p.id
            );
        }
    }

    #[test]
    fn map_edits_and_round_trips_through_json() {
        let mut m = PedalMap::profile(Profile::by_id("pageUpDown").unwrap());
        assert!(m.set(Chord::parse("B").unwrap(), Some(Action::Next)));
        assert!(m.set(Chord::parse("PageUp").unwrap(), None));
        assert!(m.set(Chord::parse("PageDown").unwrap(), Some(Action::Last)));
        let json = serde_json::to_string(&m).unwrap();
        let back: PedalMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.action(&Chord::parse("B").unwrap()), Some(Action::Next));
        assert_eq!(back.action(&Chord::parse("PageUp").unwrap()), None);
        assert_eq!(back.keys_for(Action::Last).len(), 1);
        assert!(serde_json::from_str::<PedalMap>(r#"{"Hyper+X":"next"}"#).is_err());
        assert!(serde_json::from_str::<PedalMap>(r#"{"X":"dance"}"#).is_err());
    }

    #[test]
    fn map_is_capped() {
        let mut m = PedalMap::empty();
        for i in 0..MAX_ENTRIES {
            assert!(m.set(Chord::parse(&format!("K{i}")).unwrap(), Some(Action::Next)));
        }
        assert!(!m.set(Chord::parse("Z").unwrap(), Some(Action::Next)));
        // Changing an existing key still works when full.
        assert!(m.set(Chord::parse("K0").unwrap(), Some(Action::First)));
    }

    #[test]
    fn gate_drops_repeats_and_bounces() {
        let mut g = KeyGate::new(DEFAULT_DEBOUNCE_MS);
        let pd = Chord::parse("PageDown").unwrap();
        let pu = Chord::parse("PageUp").unwrap();
        assert!(g.press(&pd, false, 1000));
        assert!(!g.press(&pd, true, 1500));
        assert!(!g.press(&pd, false, 1040));
        assert!(g.press(&pd, false, 1200));
        assert!(g.press(&pu, false, 1210));
        // A clock that goes backwards never blocks a press.
        assert!(g.press(&pu, false, 5));
    }
}
