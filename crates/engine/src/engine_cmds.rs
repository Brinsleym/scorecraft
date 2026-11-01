//! Commands about the engine itself (tagged `ours`: no enScore equivalent). They exist from M0
//! so every frontend can list what it can drive (`docs/control-protocol.md`, `command_list`).

use serde_json::{Value, json};

use crate::commands::{CommandSpec, always, opt_str, params_object, reject_unknown_keys};
use crate::{EngineError, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "engine.commands.list",
            label: "List Commands",
            menu: "",
            shortcut: None,
            params: r#"{"filter":string?}"#,
            ours: true,
            enabled: always,
            run: list,
        },
        CommandSpec {
            id: "engine.version",
            label: "About ScoreCraft",
            menu: "Help",
            shortcut: None,
            params: "{}",
            ours: true,
            enabled: always,
            run: version,
        },
    ]
}

fn list(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "engine.commands.list";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["filter"])?;
    let filter = opt_str(ID, p, "filter")?.unwrap_or("");
    let commands: Vec<Value> = session
        .commands()
        .into_iter()
        .filter(|c| c.id.starts_with(filter))
        .map(|c| c.to_json())
        .collect();
    Ok(json!({ "commands": commands }))
}

fn version(_: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "engine.version";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    Ok(json!({ "name": "ScoreCraft", "version": env!("CARGO_PKG_VERSION") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_filters_by_prefix() {
        let mut s = Session::new();
        let all = s.dispatch("engine.commands.list", &Value::Null).unwrap();
        assert_eq!(
            all["commands"].as_array().unwrap().len(),
            s.commands().len()
        );
        let ink = s
            .dispatch("engine.commands.list", &json!({"filter": "annotate."}))
            .unwrap();
        let ink = ink["commands"].as_array().unwrap();
        assert!(!ink.is_empty());
        assert!(
            ink.iter()
                .all(|c| c["id"].as_str().unwrap().starts_with("annotate."))
        );
        let none = s
            .dispatch("engine.commands.list", &json!({"filter": "sync."}))
            .unwrap();
        assert!(none["commands"].as_array().unwrap().is_empty());
    }

    #[test]
    fn list_rejects_bad_filter_and_unknown_keys() {
        let mut s = Session::new();
        let bad = s
            .dispatch("engine.commands.list", &json!({"filter": 3}))
            .unwrap_err();
        assert_eq!(bad.code(), "bad_params");
        let typo = s
            .dispatch("engine.commands.list", &json!({"filtr": "x"}))
            .unwrap_err();
        assert_eq!(typo.code(), "bad_params");
    }

    #[test]
    fn version_reports_product_name() {
        let mut s = Session::new();
        let v = s.dispatch("engine.version", &json!({})).unwrap();
        assert_eq!(v["name"], "ScoreCraft");
    }
}
