//! The command registry (`docs/architecture.md` §4.2).
//!
//! Commands live in area modules (`<area>_cmds.rs`) that expose `specs()`; each is registered
//! with one `v.extend(...)` line in [`registry`].

use serde_json::{Value, json};

use crate::{EngineError, Session};

/// Validates params and runs the command. Returns the command's JSON result.
pub type RunFn = fn(&mut Session, &Value) -> Result<Value, EngineError>;
/// Whether the command can run in the session's current state.
pub type EnabledFn = fn(&Session) -> bool;

/// One engine command.
#[derive(Clone, Copy)]
pub struct CommandSpec {
    /// Stable id from `docs/feature-map.md` (`view.turn.next`), or a descriptive id for `ours`.
    pub id: &'static str,
    /// Menu and palette label.
    pub label: &'static str,
    /// Menu path, `>`-separated (`View>Turn`); empty for commands with no menu item.
    pub menu: &'static str,
    /// Default shortcut (`Ctrl+Z`), if any.
    pub shortcut: Option<&'static str>,
    /// Params doc, as agents read it: `{"mode":"single|twoPage"="single"}`; `{}` for none.
    pub params: &'static str,
    /// True for commands with no enScore equivalent; they don't count towards parity.
    pub ours: bool,
    pub enabled: EnabledFn,
    pub run: RunFn,
}

impl std::fmt::Debug for CommandSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandSpec")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// A command's description plus whether it is enabled now (what `commands --json` prints).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub menu: &'static str,
    pub shortcut: Option<&'static str>,
    pub params: &'static str,
    pub ours: bool,
    pub enabled: bool,
}

impl CommandInfo {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "menu": self.menu,
            "shortcut": self.shortcut,
            "params": self.params,
            "ours": self.ours,
            "enabled": self.enabled,
        })
    }
}

/// Always enabled.
pub fn always(_: &Session) -> bool {
    true
}

/// Every registered command, in registration order.
pub fn registry() -> Vec<CommandSpec> {
    let mut v = Vec::new();
    v.extend(crate::engine_cmds::specs());
    v.extend(crate::score_cmds::specs());
    v.extend(crate::view_cmds::specs());
    v.extend(crate::reader_cmds::specs());
    v.extend(crate::input_cmds::specs());
    v.extend(crate::snapshot::specs());
    v.extend(crate::library_cmds::specs());
    v.extend(crate::page_cmds::specs());
    v.extend(crate::list_cmds::specs());
    v.extend(crate::edit_cmds::specs());
    v.extend(crate::annotate_cmds::specs());
    v.extend(crate::layer_cmds::specs());
    v.extend(crate::text_cmds::specs());
    v.extend(crate::stamp_cmds::specs());
    v.extend(crate::select_cmds::specs());
    v.extend(crate::export_cmds::specs());
    v.extend(crate::find_cmds::specs());
    v.extend(crate::cover_cmds::specs());
    v
}

/// Params must be a JSON object or `null` (no params). Returns the object, empty for `null`.
pub fn params_object<'a>(
    command: &str,
    params: &'a Value,
) -> Result<Option<&'a serde_json::Map<String, Value>>, EngineError> {
    match params {
        Value::Null => Ok(None),
        Value::Object(map) => Ok(Some(map)),
        other => Err(EngineError::BadParams {
            command: command.to_owned(),
            message: format!("params must be an object, got {}", kind(other)),
        }),
    }
}

/// Rejects any params key not in `allowed`, so typos fail loudly instead of being ignored.
pub fn reject_unknown_keys(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    allowed: &[&str],
) -> Result<(), EngineError> {
    if let Some(key) = params
        .into_iter()
        .flat_map(|m| m.keys())
        .find(|k| !allowed.contains(&k.as_str()))
    {
        return Err(EngineError::BadParams {
            command: command.to_owned(),
            message: format!("unknown param `{}`", truncate(key, 64)),
        });
    }
    Ok(())
}

/// Optional string param.
pub fn opt_str<'a>(
    command: &str,
    params: Option<&'a serde_json::Map<String, Value>>,
    key: &str,
) -> Result<Option<&'a str>, EngineError> {
    match params.and_then(|m| m.get(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(other) => Err(EngineError::BadParams {
            command: command.to_owned(),
            message: format!("`{key}` must be a string, got {}", kind(other)),
        }),
    }
}

/// Required string param.
pub fn req_str<'a>(
    command: &str,
    params: Option<&'a serde_json::Map<String, Value>>,
    key: &str,
) -> Result<&'a str, EngineError> {
    opt_str(command, params, key)?.ok_or_else(|| EngineError::BadParams {
        command: command.to_owned(),
        message: format!("`{key}` is required"),
    })
}

/// Optional finite number param.
pub fn opt_f64(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Result<Option<f64>, EngineError> {
    match params.and_then(|m| m.get(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => match n.as_f64() {
            Some(x) if x.is_finite() => Ok(Some(x)),
            _ => Err(bad(command, format!("`{key}` must be a finite number"))),
        },
        Some(other) => Err(bad(
            command,
            format!("`{key}` must be a number, got {}", kind(other)),
        )),
    }
}

/// Optional whole-number param in `min..=max`.
pub fn opt_int(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    key: &str,
    min: u64,
    max: u64,
) -> Result<Option<u64>, EngineError> {
    match params.and_then(|m| m.get(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => match n.as_u64() {
            Some(x) if (min..=max).contains(&x) => Ok(Some(x)),
            _ => Err(bad(
                command,
                format!("`{key}` must be a whole number from {min} to {max}"),
            )),
        },
        Some(other) => Err(bad(
            command,
            format!("`{key}` must be a number, got {}", kind(other)),
        )),
    }
}

/// Optional boolean param.
pub fn opt_bool(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Result<Option<bool>, EngineError> {
    match params.and_then(|m| m.get(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(other) => Err(bad(
            command,
            format!("`{key}` must be a boolean, got {}", kind(other)),
        )),
    }
}

/// Required id param (`"score": "0190…"`), parsed as a UUID.
pub fn req_id<T: std::str::FromStr<Err = String>>(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Result<T, EngineError> {
    let s = req_str(command, params, key)?;
    s.parse::<T>()
        .map_err(|e| bad(command, format!("`{key}`: {e}")))
}

/// Optional id param; `null` and absent are `None`.
pub fn opt_id<T: std::str::FromStr<Err = String>>(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Result<Option<T>, EngineError> {
    match opt_str(command, params, key)? {
        None => Ok(None),
        Some(s) => s
            .parse::<T>()
            .map(Some)
            .map_err(|e| bad(command, format!("`{key}`: {e}"))),
    }
}

/// Required whole-number param in `min..=max`.
pub fn req_int(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    key: &str,
    min: u64,
    max: u64,
) -> Result<u64, EngineError> {
    opt_int(command, params, key, min, max)?
        .ok_or_else(|| bad(command, format!("`{key}` is required")))
}

/// Optional list of strings, at most `max` long.
pub fn opt_str_list(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    key: &str,
    max: usize,
) -> Result<Option<Vec<String>>, EngineError> {
    match params.and_then(|m| m.get(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => {
            if items.len() > max {
                return Err(bad(command, format!("`{key}` has more than {max} items")));
            }
            items
                .iter()
                .map(|v| match v {
                    Value::String(s) => Ok(s.clone()),
                    other => Err(bad(
                        command,
                        format!("`{key}` items must be strings, got {}", kind(other)),
                    )),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        }
        Some(other) => Err(bad(
            command,
            format!("`{key}` must be a list of strings, got {}", kind(other)),
        )),
    }
}

/// Optional list of whole numbers in `min..=max`, at most `max_len` long.
pub fn opt_int_list(
    command: &str,
    params: Option<&serde_json::Map<String, Value>>,
    key: &str,
    (min, max): (u64, u64),
    max_len: usize,
) -> Result<Option<Vec<u64>>, EngineError> {
    match params.and_then(|m| m.get(key)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => {
            if items.len() > max_len {
                return Err(bad(
                    command,
                    format!("`{key}` has more than {max_len} items"),
                ));
            }
            items
                .iter()
                .map(|v| match v.as_u64() {
                    Some(x) if (min..=max).contains(&x) => Ok(x),
                    _ => Err(bad(
                        command,
                        format!("`{key}` items must be whole numbers from {min} to {max}"),
                    )),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        }
        Some(other) => Err(bad(
            command,
            format!("`{key}` must be a list of numbers, got {}", kind(other)),
        )),
    }
}

/// A string param no longer than `max` characters (names, titles).
pub fn check_len(command: &str, key: &str, s: &str, max: usize) -> Result<(), EngineError> {
    if s.chars().count() > max {
        return Err(bad(
            command,
            format!("`{key}` is longer than {max} characters"),
        ));
    }
    Ok(())
}

/// Sets `key` on a JSON object result (no-op on anything else).
pub fn set(v: &mut Value, key: &str, value: Value) {
    if let Some(o) = v.as_object_mut() {
        o.insert(key.to_owned(), value);
    }
}

/// A bad-params error for `command`.
pub fn bad(command: &str, message: String) -> EngineError {
    EngineError::BadParams {
        command: command.to_owned(),
        message,
    }
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Shortens hostile input before it goes into an error message (char-boundary safe).
pub fn truncate(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().nth(max_chars).is_some() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_well_formed() {
        let specs = registry();
        let mut ids: Vec<&str> = specs.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate command id");
        for id in ids {
            let segments: Vec<&str> = id.split('.').collect();
            assert!(segments.len() >= 2, "{id}: needs area.noun[.verb]");
            for seg in segments {
                let first = seg.chars().next().unwrap();
                assert!(
                    first.is_ascii_lowercase(),
                    "{id}: segment `{seg}` must be lowerCamelCase"
                );
                assert!(
                    seg.chars().all(|c| c.is_ascii_alphanumeric()),
                    "{id}: segment `{seg}`"
                );
            }
        }
    }

    #[test]
    fn every_spec_documents_its_params() {
        for spec in registry() {
            assert!(
                spec.params.starts_with('{') && spec.params.ends_with('}'),
                "{}",
                spec.id
            );
            assert!(!spec.label.is_empty(), "{}", spec.id);
        }
    }

    #[test]
    fn params_must_be_object_or_null() {
        assert!(params_object("x.y", &Value::Null).unwrap().is_none());
        assert!(params_object("x.y", &json!({})).unwrap().is_some());
        let err = params_object("x.y", &json!([1])).unwrap_err();
        assert_eq!(err.code(), "bad_params");
    }

    #[test]
    fn unknown_keys_are_rejected_and_truncated() {
        let long = "k".repeat(500);
        let p = json!({ long.clone(): 1 });
        let err = reject_unknown_keys("x.y", p.as_object(), &["filter"]).unwrap_err();
        assert!(err.to_string().len() < 200);
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("ééé", 2), "éé…");
        assert_eq!(truncate("ab", 2), "ab");
    }
}
