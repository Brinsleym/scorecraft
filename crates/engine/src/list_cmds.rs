//! Setlists (feature-map LIST-1, LIST-2, LIST-3, LIST-7). Entries refer to library scores and
//! never copy them (LIST-6). Positions in params are 1-based.

use serde_json::{Map, Value, json};

use scorecraft_doc::{EntryId, EntryItem, ListEntry, ListId, PageRange, ScoreId, Setlist};
use scorecraft_ops::{Op, OpError, Transaction};

use crate::commands::{
    CommandSpec, bad, check_len, opt_f64, opt_int, opt_str, params_object, reject_unknown_keys,
    req_id, req_int, req_str,
};
use crate::library::library_open;
use crate::library_cmds::{MAX_NAME, edited};
use crate::{EngineError, Session};

/// Most entries one list holds.
pub const MAX_ENTRIES: usize = 10_000;

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, menu, params, run| CommandSpec {
        id,
        label,
        menu,
        shortcut: None,
        params,
        ours: false,
        enabled: library_open,
        run,
    };
    vec![
        spec(
            "list.new",
            "New List",
            "Lists",
            r#"{"name":string}"#,
            new as _,
        ),
        spec(
            "list.rename",
            "Rename List",
            "Lists",
            r#"{"list":id,"name":string}"#,
            rename as _,
        ),
        spec(
            "list.delete",
            "Delete List",
            "Lists",
            r#"{"list":id}"#,
            delete as _,
        ),
        spec("list.get", "List Details", "", r#"{"list":id}"#, get as _),
        spec(
            "list.entry.add",
            "Add Scores to List",
            "Lists",
            r#"{"list":id,"score":id,"at":int?,"note":string?}"#,
            entry_add as _,
        ),
        spec(
            "list.entry.addPlaceholder",
            "Add Placeholder",
            "Lists",
            r#"{"list":id,"label":string,"at":int?,"note":string?}"#,
            entry_add_placeholder as _,
        ),
        spec(
            "list.entry.move",
            "Move Entry",
            "Lists",
            r#"{"list":id,"from":int,"to":int}"#,
            entry_move as _,
        ),
        spec(
            "list.entry.remove",
            "Remove from List",
            "Lists",
            r#"{"list":id,"entry":id}"#,
            entry_remove as _,
        ),
        spec(
            "list.entry.pageRange.set",
            "Entry Page Range…",
            "Lists",
            r#"{"list":id,"entry":id,"first":int?,"last":int?}"#,
            entry_page_range as _,
        ),
        spec(
            "list.entry.tempo.set",
            "Entry Tempo…",
            "Lists",
            r#"{"list":id,"entry":id,"bpm":number|null}"#,
            entry_tempo as _,
        ),
    ]
}

fn name_param(id: &str, p: Option<&Map<String, Value>>, key: &str) -> Result<String, EngineError> {
    let name = req_str(id, p, key)?.trim().to_owned();
    if name.is_empty() {
        return Err(bad(id, format!("`{key}` must not be empty")));
    }
    check_len(id, key, &name, MAX_NAME)?;
    Ok(name)
}

fn list(l: &crate::library::LibrarySession, id: ListId) -> Result<&Setlist, EngineError> {
    l.lib.lists.get(&id).ok_or(OpError::NoList(id).into())
}

fn entry(list: &Setlist, id: EntryId) -> Result<&ListEntry, EngineError> {
    list.entries
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| OpError::Missing(format!("entry {id}")).into())
}

fn new(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.new";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["name"])?;
    let name = name_param(ID, p, "name")?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let id: ListId = l.ids.id(now);
    l.commit(
        "New List",
        Transaction::single(Op::InsertList {
            list: Box::new(Setlist {
                id,
                name,
                entries: Vec::new(),
                modified_ms: now,
            }),
        }),
    )?;
    Ok(edited(l, json!({ "list": id.to_string() })))
}

fn rename(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.rename";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["list", "name"])?;
    let id: ListId = req_id(ID, p, "list")?;
    let name = name_param(ID, p, "name")?;
    let l = session.lib_mut(ID)?;
    l.commit(
        "Rename List",
        Transaction::single(Op::RenameList { list: id, name }),
    )?;
    Ok(edited(l, json!({})))
}

fn delete(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.delete";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["list"])?;
    let id: ListId = req_id(ID, p, "list")?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let pages: Vec<ScoreId> = list(l, id)?
        .entries
        .iter()
        .filter_map(|e| match e.item {
            EntryItem::Placeholder { page, .. } => page,
            EntryItem::Score { .. } => None,
        })
        .collect();
    let mut ops = vec![Op::RemoveList { list: id }];
    for page in pages {
        ops.extend(trash_page(l, page, now));
    }
    l.commit("Delete List", Transaction::new(ops))?;
    Ok(edited(l, json!({})))
}

fn get(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.get";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["list"])?;
    let id: ListId = req_id(ID, p, "list")?;
    let l = session.lib_mut(ID)?;
    let x = list(l, id)?;
    let entries: Vec<Value> = x
        .entries
        .iter()
        .map(|e| {
            let mut v = serde_json::to_value(e).unwrap_or(Value::Null);
            if let Some(s) = e.score().and_then(|s| l.lib.scores.get(&s)) {
                crate::commands::set(&mut v, "title", json!(s.display_title()));
            }
            v
        })
        .collect();
    Ok(json!({ "id": x.id.to_string(), "name": x.name, "entries": entries }))
}

/// Sends a placeholder's page to Recently Deleted, unless it is gone or there already.
fn trash_page(l: &crate::library::LibrarySession, page: ScoreId, now: u64) -> Option<Op> {
    (l.lib.scores.contains_key(&page) && !l.lib.is_trashed(page)).then_some(Op::Trash {
        score: page,
        at_ms: now,
    })
}

/// A 1-based insert position (`at`), default the end.
fn insert_at(id: &str, p: Option<&Map<String, Value>>, len: usize) -> Result<usize, EngineError> {
    if len >= MAX_ENTRIES {
        return Err(bad(
            id,
            format!("a list holds at most {MAX_ENTRIES} entries"),
        ));
    }
    Ok(match opt_int(id, p, "at", 1, len as u64 + 1)? {
        Some(n) => (n - 1) as usize,
        None => len,
    })
}

/// The A4 template a placeholder's page is made from, in points.
const A4: (f64, f64) = (595.28, 841.89);
/// The longest placeholder label put on its page.
pub const MAX_PLACEHOLDER_LABEL: usize = 80;

/// A placeholder label made safe to print and to use in a file name: control and format
/// characters dropped, runs of white space made one space, at most
/// [`MAX_PLACEHOLDER_LABEL`] characters.
pub(crate) fn clean_label(raw: &str) -> String {
    let kept: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control() && !is_format_char(*c))
        .collect();
    let words: Vec<&str> = kept.split(' ').filter(|w| !w.is_empty()).collect();
    words
        .join(" ")
        .chars()
        .take(MAX_PLACEHOLDER_LABEL)
        .collect()
}

/// Invisible characters that change how text reads (bidi overrides, zero-width joiners).
fn is_format_char(c: char) -> bool {
    matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}')
}

/// The label as a file name: characters no file system allows become `_`.
fn file_name_for(label: &str) -> String {
    let stem: String = label
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    format!("{}.pdf", stem.trim_matches(['.', ' ']))
}

/// A placeholder's page: a hidden library score from the blank A4 template with `label` as a
/// large text note centred on it, which the player can move, edit or write around.
fn placeholder_page(
    id: &str,
    l: &mut crate::library::LibrarySession,
    label: &str,
    now: u64,
) -> Result<scorecraft_doc::Score, EngineError> {
    use scorecraft_doc::{Annotation, AnnotationKind, PageKey, TextNote, TextStyle};
    use scorecraft_geom::{Point, Rect};
    let pdf = scorecraft_pdf::manuscript(scorecraft_pdf::Paper::Blank, A4.0, A4.1, 1, label)?;
    let mut score = crate::library_cmds::import_bytes(l, pdf, &file_name_for(label), None, now)?;
    score.metadata.title = label.to_owned();
    let page = Rect::new(0.0, 0.0, A4.0, A4.1);
    let style = TextStyle {
        size: crate::markup::TEXT_SIZES.max,
        ..TextStyle::default()
    };
    let laid = crate::text_cmds::place(label, &style, Point::new(0.0, A4.1), page)?;
    let (x0, y1) = ((A4.0 - laid.width()) / 2.0, (A4.1 + laid.height()) / 2.0);
    let rect = Rect::new(x0, y1 - laid.height(), x0 + laid.width(), y1);
    let layer = score
        .layers
        .first()
        .map(|x| x.id)
        .ok_or_else(|| bad(id, "the new page has no layer".into()))?;
    l.hlc = l.hlc.tick(now);
    score.annotations.insert(
        PageKey(0),
        vec![Annotation {
            id: l.ids.id(now),
            layer,
            stamp: l.hlc,
            kind: AnnotationKind::Text(TextNote {
                rect,
                text: label.to_owned(),
                style,
            }),
        }],
    );
    Ok(score)
}

fn add(
    session: &mut Session,
    id: &str,
    params: &Value,
    placeholder: bool,
) -> Result<Value, EngineError> {
    let p = params_object(id, params)?;
    let item_key = if placeholder { "label" } else { "score" };
    reject_unknown_keys(id, p, &["list", item_key, "at", "note"])?;
    let list_id: ListId = req_id(id, p, "list")?;
    let note = opt_str(id, p, "note")?.unwrap_or("").to_owned();
    check_len(id, "note", &note, MAX_NAME)?;
    let label = if placeholder {
        let label = clean_label(&name_param(id, p, "label")?);
        if label.is_empty() {
            return Err(bad(id, "`label` must have some visible text".into()));
        }
        Some(label)
    } else {
        None
    };
    let score_id = if placeholder {
        None
    } else {
        Some(req_id::<ScoreId>(id, p, "score")?)
    };
    let now = session.now_ms();
    let l = session.lib_mut(id)?;
    let index = insert_at(id, p, list(l, list_id)?.entries.len())?;
    let mut ops = Vec::new();
    let mut out = json!({});
    let item = match (label, score_id) {
        (Some(label), _) => {
            let page = placeholder_page(id, l, &label, now)?;
            crate::commands::set(&mut out, "page", json!(page.id.to_string()));
            let page_id = page.id;
            ops.push(Op::InsertScore {
                score: Box::new(page),
            });
            EntryItem::Placeholder {
                label,
                page: Some(page_id),
            }
        }
        (None, Some(score)) => {
            // The library's page range comes with the score, once: after that the entry's range
            // and the library's are independent (owner, 2026-11-01). The answer says so, for the
            // shell's notice.
            let page_range = l.lib.scores.get(&score).and_then(|s| s.page_range);
            if let Some(r) = page_range {
                crate::commands::set(
                    &mut out,
                    "pageRange",
                    json!({ "first": r.first + 1, "last": r.last + 1, "fromLibrary": true }),
                );
            }
            EntryItem::Score {
                score,
                page_range,
                tempo: None,
            }
        }
        (None, None) => return Err(bad(id, "`score` is required".into())),
    };
    let eid: EntryId = l.ids.id(now);
    ops.push(Op::InsertEntry {
        list: list_id,
        index,
        entry: ListEntry {
            id: eid,
            item,
            note,
        },
    });
    l.commit(
        if placeholder {
            "Add Placeholder"
        } else {
            "Add to List"
        },
        Transaction::new(ops),
    )?;
    crate::commands::set(&mut out, "entry", json!(eid.to_string()));
    Ok(edited(l, out))
}

fn entry_add(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    add(session, "list.entry.add", params, false)
}

fn entry_add_placeholder(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    add(session, "list.entry.addPlaceholder", params, true)
}

fn entry_move(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.entry.move";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["list", "from", "to"])?;
    let id: ListId = req_id(ID, p, "list")?;
    let l = session.lib_mut(ID)?;
    let n = (list(l, id)?.entries.len() as u64).max(1);
    let from = (req_int(ID, p, "from", 1, n)? - 1) as usize;
    let to = (req_int(ID, p, "to", 1, n)? - 1) as usize;
    l.commit(
        "Move Entry",
        Transaction::single(Op::MoveEntry { list: id, from, to }),
    )?;
    Ok(edited(l, json!({})))
}

fn entry_remove(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.entry.remove";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["list", "entry"])?;
    let id: ListId = req_id(ID, p, "list")?;
    let e: EntryId = req_id(ID, p, "entry")?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let mut ops = vec![Op::RemoveEntry { list: id, entry: e }];
    // A placeholder's page goes to Recently Deleted with it, so its ink can still be restored.
    if let EntryItem::Placeholder {
        page: Some(page), ..
    } = entry(list(l, id)?, e)?.item
    {
        ops.extend(trash_page(l, page, now));
    }
    l.commit("Remove from List", Transaction::new(ops))?;
    Ok(edited(l, json!({})))
}

fn entry_page_range(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.entry.pageRange.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["list", "entry", "first", "last"])?;
    let id: ListId = req_id(ID, p, "list")?;
    let eid: EntryId = req_id(ID, p, "entry")?;
    let l = session.lib_mut(ID)?;
    let mut e = entry(list(l, id)?, eid)?.clone();
    let EntryItem::Score { score, tempo, .. } = e.item else {
        return Err(bad(ID, "a placeholder has no pages".into()));
    };
    let n = l.lib.scores.get(&score).map_or(0, |s| s.pages.len()) as u64;
    let first = opt_int(ID, p, "first", 1, n.max(1))?;
    let last = opt_int(ID, p, "last", 1, n.max(1))?;
    let page_range = match (first, last) {
        (None, None) => None,
        (f, la) => Some(
            PageRange::new(
                (f.unwrap_or(1) - 1) as u32,
                (la.unwrap_or(n.max(1)) - 1) as u32,
            )
            .ok_or_else(|| bad(ID, "`first` must not be after `last`".into()))?,
        ),
    };
    e.item = EntryItem::Score {
        score,
        page_range,
        tempo,
    };
    l.commit(
        "Entry Page Range",
        Transaction::single(Op::SetEntry { list: id, entry: e }),
    )?;
    Ok(edited(l, json!({})))
}

fn entry_tempo(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "list.entry.tempo.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["list", "entry", "bpm"])?;
    let id: ListId = req_id(ID, p, "list")?;
    let eid: EntryId = req_id(ID, p, "entry")?;
    let bpm = opt_f64(ID, p, "bpm")?;
    if let Some(b) = bpm
        && !(b > 0.0 && b <= 1000.0)
    {
        return Err(bad(ID, "`bpm` must be above 0 and at most 1000".into()));
    }
    let l = session.lib_mut(ID)?;
    let mut e = entry(list(l, id)?, eid)?.clone();
    let EntryItem::Score {
        score, page_range, ..
    } = e.item
    else {
        return Err(bad(ID, "a placeholder has no tempo".into()));
    };
    e.item = EntryItem::Score {
        score,
        page_range,
        tempo: bpm.map(|b| b as f32),
    };
    l.commit(
        "Entry Tempo",
        Transaction::single(Op::SetEntry { list: id, entry: e }),
    )?;
    Ok(edited(l, json!({})))
}
