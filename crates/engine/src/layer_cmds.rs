//! Annotation layers of the open score (feature-map ANNO-10, spec `docs/spec/ink.md`): add,
//! rename, show or hide, lock, reorder, delete and choose the active one. Layers draw bottom
//! first in list order; hidden layers aren't drawn; a locked layer is drawn but can't be
//! changed. Every change is one undo step in the score's history.

use serde_json::{Map, Value, json};

use scorecraft_doc::{Layer, LayerId, Score, ScoreId};
use scorecraft_ops::{Op, OpError, Transaction, edit};

use crate::annotate_cmds::markup_ready;
use crate::commands::{
    CommandSpec, bad, check_len, opt_bool, opt_str, params_object, reject_unknown_keys, req_id,
    req_int, req_str, truncate,
};
use crate::{EngineError, Session};

/// Longest layer name.
const MAX_LAYER_NAME: usize = 64;
/// Most layers one score keeps.
pub const MAX_LAYERS: usize = 64;

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, params, ours, run| CommandSpec {
        id,
        label,
        menu: "Markup>Layers",
        shortcut: None,
        params,
        ours,
        enabled: markup_ready,
        run,
    };
    vec![
        spec(
            "annotate.layer.new",
            "New Layer",
            r#"{"name":string?}"#,
            false,
            layer_new as _,
        ),
        spec(
            "annotate.layer.setVisible",
            "Show Layer",
            r#"{"layer":id,"visible":bool?}"#,
            false,
            layer_visible as _,
        ),
        spec(
            "annotate.layer.rename",
            "Rename Layer",
            r#"{"layer":id,"name":string}"#,
            true,
            layer_rename as _,
        ),
        spec(
            "annotate.layer.setLocked",
            "Lock Layer",
            r#"{"layer":id,"locked":bool?}"#,
            true,
            layer_locked as _,
        ),
        spec(
            "annotate.layer.move",
            "Move Layer",
            r#"{"layer":id,"to":int}"#,
            true,
            layer_move as _,
        ),
        spec(
            "annotate.layer.delete",
            "Delete Layer",
            r#"{"layer":id}"#,
            true,
            layer_delete as _,
        ),
        spec(
            "annotate.layer.setActive",
            "Active Layer",
            r#"{"layer":id}"#,
            true,
            layer_active as _,
        ),
        spec("annotate.layer.list", "Layers", "{}", true, layer_list as _),
    ]
}

/// The open score's library id.
fn open_score(session: &Session, command: &str) -> Result<ScoreId, EngineError> {
    session
        .score
        .as_ref()
        .and_then(|o| o.library_score)
        .ok_or_else(|| EngineError::Disabled(command.to_owned()))
}

fn score(session: &Session, id: ScoreId) -> Result<&Score, EngineError> {
    session
        .library()
        .and_then(|l| l.scores.get(&id))
        .ok_or(OpError::NoScore(id).into())
}

/// The layer named by the `layer` param, from the open score.
fn layer_param(
    session: &Session,
    command: &str,
    p: Option<&Map<String, Value>>,
) -> Result<(ScoreId, Layer), EngineError> {
    let id = open_score(session, command)?;
    let layer: LayerId = req_id(command, p, "layer")?;
    let l = score(session, id)?
        .layer(layer)
        .cloned()
        .ok_or_else(|| OpError::Missing(format!("layer {layer}")))?;
    Ok((id, l))
}

/// A name param, trimmed, non-empty, at most [`MAX_LAYER_NAME`] characters.
fn name(command: &str, raw: &str) -> Result<String, EngineError> {
    check_len(command, "name", raw, MAX_LAYER_NAME)?;
    let n = raw.trim();
    if n.is_empty() {
        return Err(bad(command, "`name` can't be empty".into()));
    }
    Ok(n.to_owned())
}

/// The layers of the open score, bottom first, as `annotate.layer.list` returns them.
fn list_json(session: &Session, id: ScoreId) -> Result<Value, EngineError> {
    let s = score(session, id)?;
    let active = session.markup.layer(s);
    let layers: Vec<Value> = s
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let markings = s
                .annotations
                .values()
                .flatten()
                .filter(|a| a.layer == l.id)
                .count();
            json!({
                "layer": l.id.to_string(),
                "position": i + 1,
                "name": l.name,
                "visible": l.visible,
                "locked": l.locked,
                "active": Some(l.id) == active,
                "markings": markings,
            })
        })
        .collect();
    Ok(json!({
        "layers": layers,
        "active": active.map(|a| a.to_string()),
    }))
}

/// Commits `tx` as one step and returns the layer list.
fn commit(
    session: &mut Session,
    command: &str,
    id: ScoreId,
    label: &str,
    tx: Transaction,
) -> Result<Value, EngineError> {
    session.lib_mut(command)?.commit(label, tx)?;
    list_json(session, id)
}

fn layer_new(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.layer.new";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["name"])?;
    let id = open_score(session, ID)?;
    let s = score(session, id)?;
    if s.layers.len() >= MAX_LAYERS {
        return Err(OpError::Invalid(format!("a score keeps at most {MAX_LAYERS} layers")).into());
    }
    let n = s.layers.len();
    let name = match opt_str(ID, p, "name")? {
        Some(raw) => name(ID, raw)?,
        None => {
            // "Layer N", the first number no layer uses.
            let mut k = n + 1;
            while s.layers.iter().any(|l| l.name == format!("Layer {k}")) {
                k += 1;
            }
            format!("Layer {k}")
        }
    };
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let layer = Layer {
        id: l.ids.id(now),
        name,
        visible: true,
        locked: false,
    };
    let new_id = layer.id;
    l.commit(
        "New Layer",
        Transaction::single(Op::InsertLayer {
            score: id,
            index: n,
            layer,
        }),
    )?;
    // A new layer is where the next markings go.
    session.markup.layers.insert(id, new_id);
    let mut out = list_json(session, id)?;
    crate::commands::set(&mut out, "layer", json!(new_id.to_string()));
    Ok(out)
}

/// Sets a layer flag; no `value` toggles it.
fn set_flag(
    session: &mut Session,
    command: &str,
    params: &Value,
    key: &str,
    label: &str,
    apply: fn(&mut Layer, Option<bool>),
) -> Result<Value, EngineError> {
    let p = params_object(command, params)?;
    reject_unknown_keys(command, p, &["layer", key])?;
    let value = opt_bool(command, p, key)?;
    let (id, mut layer) = layer_param(session, command, p)?;
    apply(&mut layer, value);
    commit(
        session,
        command,
        id,
        label,
        Transaction::single(Op::SetLayer { score: id, layer }),
    )
}

fn layer_visible(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    set_flag(
        session,
        "annotate.layer.setVisible",
        params,
        "visible",
        "Show or Hide Layer",
        |l, v| l.visible = v.unwrap_or(!l.visible),
    )
}

fn layer_locked(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    set_flag(
        session,
        "annotate.layer.setLocked",
        params,
        "locked",
        "Lock or Unlock Layer",
        |l, v| l.locked = v.unwrap_or(!l.locked),
    )
}

fn layer_rename(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.layer.rename";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["layer", "name"])?;
    let new_name = name(ID, req_str(ID, p, "name")?)?;
    let (id, mut layer) = layer_param(session, ID, p)?;
    layer.name = new_name;
    commit(
        session,
        ID,
        id,
        "Rename Layer",
        Transaction::single(Op::SetLayer { score: id, layer }),
    )
}

/// Moves a layer to 1-based position `to` (1 is the bottom, drawn first).
fn layer_move(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.layer.move";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["layer", "to"])?;
    let (id, layer) = layer_param(session, ID, p)?;
    let s = score(session, id)?;
    let n = s.layers.len();
    let to = req_int(ID, p, "to", 1, n as u64)?;
    let to = usize::try_from(to - 1).map_err(|_| bad(ID, "`to` is too large".into()))?;
    let from = s
        .layers
        .iter()
        .position(|l| l.id == layer.id)
        .ok_or_else(|| OpError::Missing(format!("layer {}", layer.id)))?;
    if from == to {
        return list_json(session, id);
    }
    commit(
        session,
        ID,
        id,
        "Move Layer",
        Transaction::single(Op::MoveLayer {
            score: id,
            from,
            to,
        }),
    )
}

/// Deletes a layer and its markings as one undo step. The last layer can't be deleted.
fn layer_delete(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.layer.delete";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["layer"])?;
    let (id, layer) = layer_param(session, ID, p)?;
    let lib = session
        .library()
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    if score(session, id)?.layers.len() <= 1 {
        return Err(OpError::Invalid("a score keeps at least one layer".into()).into());
    }
    let tx = edit::remove_layer(lib, id, layer.id)?;
    let label = format!("Delete Layer “{}”", truncate(&layer.name, 32));
    session.lib_mut(ID)?.commit(&label, tx)?;
    // The active layer falls back to the top one (undo brings the layer back, not the choice).
    if session.markup.layers.get(&id) == Some(&layer.id) {
        session.markup.layers.remove(&id);
    }
    list_json(session, id)
}

/// Chooses the layer new markings go into. Not an undo step: it changes no marking.
fn layer_active(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.layer.setActive";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["layer"])?;
    let (id, layer) = layer_param(session, ID, p)?;
    session.markup.layers.insert(id, layer.id);
    list_json(session, id)
}

fn layer_list(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.layer.list";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &[])?;
    let id = open_score(session, ID)?;
    list_json(session, id)
}

#[cfg(test)]
mod tests;
