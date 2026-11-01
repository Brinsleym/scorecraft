//! Playing a setlist (feature-map LIST-4, spec `docs/spec/performance.md`): its scores in turn,
//! each within its entry's page range, a turn past the last page opening the next entry at its
//! first page (and back past the first, the previous entry at its last page). Breaks
//! (placeholders) are passed over and reported, so the shell can show them. The same score can
//! be in many lists and several times in one (LIST-6): entries refer to it, never copy it.

use serde_json::{Value, json};

use scorecraft_doc::{EntryItem, ListId, PageRange, ScoreId};

use crate::commands::{
    CommandSpec, always, bad, opt_int, params_object, reject_unknown_keys, req_id,
};
use crate::library::library_open;
use crate::{EngineError, Session};

/// The setlist being played and where in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playback {
    pub list: ListId,
    /// 0-based entry.
    pub entry: usize,
    pub score: ScoreId,
    /// The entry's page range (positions in the score's pages), if it has one.
    pub range: Option<PageRange>,
    pub tempo: Option<f32>,
}

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, menu, params, ours, enabled, run| CommandSpec {
        id,
        label,
        menu,
        shortcut: None,
        params,
        ours,
        enabled,
        run,
    };
    vec![
        spec(
            "list.play",
            "Play List",
            "Lists",
            r#"{"list":id,"entry":int=1}"#,
            false,
            library_open,
            play as _,
        ),
        spec(
            "list.stop",
            "Stop Playing List",
            "Lists",
            "{}",
            true,
            always,
            stop as _,
        ),
        spec("list.state", "Playing", "", "{}", true, always, state as _),
    ]
}

/// An entry found to play: its index, score, page range, tempo, and the breaks passed.
type Found = (usize, ScoreId, Option<PageRange>, Option<f32>, Vec<String>);

/// Which way to look for the next entry with a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Way {
    Forward,
    Back,
}

/// The entry with a score at or past `from` going `way`, and the breaks passed on the way.
fn find(
    session: &Session,
    list: ListId,
    from: usize,
    way: Way,
) -> Result<Option<Found>, EngineError> {
    let lib = session
        .library()
        .ok_or_else(|| EngineError::Disabled("list.play".into()))?;
    let l = lib
        .lists
        .get(&list)
        .ok_or(scorecraft_ops::OpError::NoList(list))?;
    let mut breaks = Vec::new();
    let mut i = from;
    loop {
        let Some(e) = l.entries.get(i) else {
            return Ok(None);
        };
        match &e.item {
            EntryItem::Score {
                score,
                page_range,
                tempo,
            } if lib.scores.contains_key(score) && !lib.is_trashed(*score) => {
                return Ok(Some((i, *score, *page_range, *tempo, breaks)));
            }
            EntryItem::Placeholder { label } => breaks.push(label.clone()),
            // A score in the trash or gone is passed over.
            EntryItem::Score { .. } => {}
        }
        match way {
            Way::Forward => i += 1,
            Way::Back => {
                let Some(j) = i.checked_sub(1) else {
                    return Ok(None);
                };
                i = j;
            }
        }
    }
}

/// Opens entry `from` (or the nearest with a score going `way`), at its first page or, with
/// `at_end`, its last. `None` when there is none that way.
fn open_entry(
    session: &mut Session,
    list: ListId,
    from: usize,
    way: Way,
    at_end: bool,
) -> Result<Option<Value>, EngineError> {
    let Some((entry, score, range, tempo, breaks)) = find(session, list, from, way)? else {
        return Ok(None);
    };
    // The next score opens in the same window, layout and turn mode as the last.
    let before = session.reader_state();
    session.open_library_score(score, None)?;
    if let Some(st) = before {
        carry_reader(session, &st);
    }
    session.playback = Some(Playback {
        list,
        entry,
        score,
        range,
        tempo,
    });
    session.refresh_reader();
    // The entry's tempo sets the metronome (LIST-3).
    if let Some(t) = tempo {
        let now = session.now_ms();
        session.metronome.set_bpm(f64::from(t), now);
    }
    if at_end {
        session.dispatch("view.page.goto", &json!({ "to": "last" }))?;
    }
    let mut v = state_json(session);
    crate::commands::set(&mut v, "breaks", json!(breaks));
    crate::commands::set(&mut v, "turned", json!(true));
    Ok(Some(v))
}

/// Applies the reader settings of `st` (an earlier `view.state`) to the reader just opened:
/// the viewport, layout, turn mode and fit. Each is best effort.
fn carry_reader(session: &mut Session, st: &Value) {
    if let Some(vp) = st.get("viewport") {
        let _ = session.dispatch("view.viewport.set", vp);
    }
    let first_alone = st
        .get("firstAlone")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let _ = match st.get("layout").and_then(Value::as_str) {
        Some("twoPage") => {
            session.dispatch("view.layout.twoPage", &json!({ "firstAlone": first_alone }))
        }
        _ => session.dispatch("view.layout.single", &json!({})),
    };
    let _ = match st.get("mode").and_then(Value::as_str) {
        Some("halfPage") => session.dispatch(
            "view.turn.halfPage",
            &json!({ "split": st.get("split").cloned().unwrap_or(json!(0.5)) }),
        ),
        Some("alternating") => session.dispatch("view.turn.alternating", &json!({})),
        _ => Ok(Value::Null),
    };
    if st.pointer("/zoom/fit").and_then(Value::as_str) == Some("width") {
        let _ = session.dispatch("view.zoom.landscapeFit", &json!({}));
    }
}

/// A turn that ran off the end (or start) of the open entry: the next (or previous) entry, if
/// a list is playing and there is one.
pub(crate) fn continue_turn(session: &mut Session, way: Way) -> Result<Option<Value>, EngineError> {
    let Some(p) = session.playback else {
        return Ok(None);
    };
    let from = match way {
        Way::Forward => p.entry + 1,
        Way::Back => match p.entry.checked_sub(1) {
            Some(i) => i,
            None => return Ok(None),
        },
    };
    open_entry(session, p.list, from, way, way == Way::Back)
}

fn state_json(session: &Session) -> Value {
    let Some(p) = session.playback else {
        return json!({ "playing": false });
    };
    let lib = session.library();
    let list = lib.and_then(|l| l.lists.get(&p.list));
    let title = |id: Option<ScoreId>| {
        id.and_then(|id| lib.and_then(|l| l.scores.get(&id)))
            .map(|s| s.display_title().to_owned())
    };
    let next = list.and_then(|l| l.entries.iter().skip(p.entry + 1).find_map(|e| e.score()));
    json!({
        "playing": true,
        "list": p.list.to_string(),
        "name": list.map(|l| l.name.clone()),
        "entry": p.entry + 1,
        "entries": list.map_or(0, |l| l.entries.len()),
        "score": p.score.to_string(),
        "title": title(Some(p.score)),
        "tempo": p.tempo,
        "next": title(next),
    })
}

fn play(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.play";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["list", "entry"])?;
    let list: ListId = req_id(ID, p, "list")?;
    let entry = opt_int(ID, p, "entry", 1, u64::from(u32::MAX))?.unwrap_or(1);
    let from = usize::try_from(entry - 1).map_err(|_| bad(ID, "`entry` is too large".into()))?;
    let count = session
        .library()
        .and_then(|l| l.lists.get(&list))
        .map(|l| l.entries.len())
        .ok_or(scorecraft_ops::OpError::NoList(list))?;
    if from >= count.max(1) {
        return Err(bad(
            ID,
            format!("`entry` must be from 1 to {}", count.max(1)),
        ));
    }
    open_entry(session, list, from, Way::Forward, false)?
        .ok_or_else(|| bad(ID, "the list has no score to play from there".into()))
}

fn stop(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.stop";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    session.playback = None;
    session.refresh_reader();
    Ok(state_json(session))
}

fn state(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.state";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    Ok(state_json(session))
}

#[cfg(test)]
mod tests;
