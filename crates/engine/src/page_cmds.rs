//! Editing a score in the library: metadata (LIB-4), pages (PAGE-1..4), page range (PAGE-8) and
//! bookmarks (LIB-11). Page numbers in params are 1-based positions in the score, as the reader
//! shows them; edits never touch the imported file.

use serde_json::{Map, Value, json};

use scorecraft_doc::{Bookmark, BookmarkId, Library, PageKey, PageRange, Score, ScoreId};
use scorecraft_geom::Rotation;
use scorecraft_ops::{Op, OpError, Transaction, edit};

use crate::commands::{
    CommandSpec, bad, check_len, opt_bool, opt_int, opt_int_list, opt_str, opt_str_list,
    params_object, reject_unknown_keys, req_id, req_int, req_str,
};
use crate::library::library_open;
use crate::library_cmds::{MAX_NAME, edited};
use crate::{EngineError, Session};

/// Longest notes field.
const MAX_NOTES: usize = 100_000;
/// Most tags or custom fields on one score.
const MAX_TAGS: usize = 200;

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, menu, params, enabled, run| CommandSpec {
        id,
        label,
        menu,
        shortcut: None,
        params,
        ours: false,
        enabled,
        run,
    };
    vec![
        spec(
            "score.metadata.set",
            "Edit Score Info…",
            "Score",
            r#"{"score":id,"title":string?,"composer":string?,"arranger":string?,"lyricist":string?,"key":string?,"genre":string?,"year":string?,"catalogue":string?,"publisher":string?,"notes":string?,"tags":[string]?,"instruments":[string]?,"difficulty":0..5?,"custom":{string:string|null}?}"#,
            library_open,
            metadata_set as _,
        ),
        spec(
            "page.move",
            "Move Page",
            "Score>Pages",
            r#"{"score":id,"from":int,"to":int}"#,
            library_open,
            page_move as _,
        ),
        spec(
            "page.rotate",
            "Rotate Page",
            "Score>Pages",
            r#"{"score":id,"page":int,"degrees":90|180|270|-90=90}"#,
            library_open,
            page_rotate as _,
        ),
        spec(
            "page.setHidden",
            "Hide Page",
            "Score>Pages",
            r#"{"score":id,"page":int,"hidden":bool=true}"#,
            library_open,
            page_hide as _,
        ),
        spec(
            "page.delete",
            "Delete Pages",
            "Score>Pages",
            r#"{"score":id,"pages":[int]}"#,
            library_open,
            page_delete as _,
        ),
        spec(
            "score.pageRange.set",
            "Page Range…",
            "Score",
            r#"{"score":id,"first":int?,"last":int?}"#,
            library_open,
            page_range_set as _,
        ),
        spec(
            "score.bookmark.add",
            "Add Bookmark",
            "Score>Bookmarks",
            r#"{"score":id,"name":string,"page":int}"#,
            library_open,
            bookmark_add as _,
        ),
        spec(
            "score.bookmark.remove",
            "Remove Bookmark",
            "Score>Bookmarks",
            r#"{"score":id,"bookmark":id}"#,
            library_open,
            bookmark_remove as _,
        ),
    ]
}

fn score(lib: &Library, id: ScoreId) -> Result<&Score, EngineError> {
    lib.scores.get(&id).ok_or(OpError::NoScore(id).into())
}

/// A 1-based page param as a 0-based position in `s`.
fn position(
    command: &str,
    p: Option<&Map<String, Value>>,
    key: &str,
    s: &Score,
) -> Result<usize, EngineError> {
    let n = req_int(command, p, key, 1, (s.pages.len() as u64).max(1))?;
    usize::try_from(n - 1).map_err(|_| bad(command, format!("`{key}` is too large")))
}

fn metadata_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "score.metadata.set";
    let p = params_object(ID, params)?;
    let fields = [
        "title",
        "composer",
        "arranger",
        "lyricist",
        "key",
        "genre",
        "year",
        "catalogue",
        "publisher",
        "notes",
    ];
    let mut allowed = vec!["score", "tags", "instruments", "difficulty", "custom"];
    allowed.extend(fields);
    reject_unknown_keys(ID, p, &allowed)?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let l = session.lib_mut(ID)?;
    let mut m = score(&l.lib, id)?.metadata.clone();
    let mut changed = Vec::new();
    for key in fields {
        if let Some(v) = opt_str(ID, p, key)? {
            check_len(
                ID,
                key,
                v,
                if key == "notes" { MAX_NOTES } else { MAX_NAME },
            )?;
            let slot = match key {
                "title" => &mut m.title,
                "composer" => &mut m.composer,
                "arranger" => &mut m.arranger,
                "lyricist" => &mut m.lyricist,
                "key" => &mut m.key,
                "genre" => &mut m.genre,
                "year" => &mut m.year,
                "catalogue" => &mut m.catalogue,
                "publisher" => &mut m.publisher,
                _ => &mut m.notes,
            };
            *slot = v.trim().to_owned();
            // A typed field is the user's, not detection's (LIB-7 won't overwrite it).
            m.detected.retain(|d| d != key);
            changed.push(key);
        }
    }
    for key in ["tags", "instruments"] {
        if let Some(list) = opt_str_list(ID, p, key, MAX_TAGS)? {
            let mut clean: Vec<String> = Vec::new();
            for t in list {
                check_len(ID, key, &t, MAX_NAME)?;
                let t = t.trim().to_owned();
                if !t.is_empty() && !clean.contains(&t) {
                    clean.push(t);
                }
            }
            if key == "tags" {
                m.tags = clean;
            } else {
                m.instruments = clean;
            }
            changed.push(key);
        }
    }
    if let Some(d) = opt_int(ID, p, "difficulty", 0, 5)? {
        m.difficulty = u8::try_from(d).unwrap_or(0);
        changed.push("difficulty");
    }
    match p.and_then(|o| o.get("custom")) {
        None | Some(Value::Null) => {}
        Some(Value::Object(c)) => {
            if c.len() > MAX_TAGS {
                return Err(bad(ID, format!("`custom` has more than {MAX_TAGS} fields")));
            }
            for (k, v) in c {
                check_len(ID, "custom", k, MAX_NAME)?;
                match v {
                    Value::Null => {
                        m.custom.remove(k);
                    }
                    Value::String(s) => {
                        check_len(ID, "custom", s, MAX_NOTES)?;
                        m.custom.insert(k.clone(), s.clone());
                    }
                    _ => return Err(bad(ID, "`custom` values must be strings or null".into())),
                }
            }
            if m.custom.len() > MAX_TAGS {
                return Err(bad(
                    ID,
                    format!("a score keeps at most {MAX_TAGS} custom fields"),
                ));
            }
            changed.push("custom");
        }
        Some(_) => return Err(bad(ID, "`custom` must be an object".into())),
    }
    if changed.is_empty() {
        return Err(bad(ID, "give at least one field to change".into()));
    }
    l.commit(
        "Edit Score Info",
        Transaction::single(Op::SetMetadata {
            score: id,
            metadata: Box::new(m),
        }),
    )?;
    Ok(edited(l, json!({ "changed": changed })))
}

fn page_move(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "page.move";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "from", "to"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let l = session.lib_mut(ID)?;
    let s = score(&l.lib, id)?;
    let (from, to) = (position(ID, p, "from", s)?, position(ID, p, "to", s)?);
    l.commit(
        "Move Page",
        Transaction::single(Op::MovePage {
            score: id,
            from,
            to,
        }),
    )?;
    Ok(edited(l, json!({})))
}

fn page_rotate(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "page.rotate";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "page", "degrees"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let degrees = match p.and_then(|o| o.get("degrees")) {
        None | Some(Value::Null) => 90,
        Some(v) => v
            .as_i64()
            .ok_or_else(|| bad(ID, "`degrees` must be a whole number".into()))?,
    };
    let by = Rotation::from_degrees(degrees)
        .ok_or_else(|| bad(ID, "`degrees` must be a multiple of 90".into()))?;
    let l = session.lib_mut(ID)?;
    let s = score(&l.lib, id)?;
    let at = position(ID, p, "page", s)?;
    let tx = edit::rotate_page(&l.lib, id, at, by)?;
    l.commit("Rotate Page", tx)?;
    let rotation = l
        .lib
        .scores
        .get(&id)
        .and_then(|s| s.pages.get(at))
        .map(|pg| pg.rotation.degrees());
    Ok(edited(l, json!({ "rotation": rotation })))
}

fn page_hide(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "page.setHidden";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "page", "hidden"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let hidden = opt_bool(ID, p, "hidden")?.unwrap_or(true);
    let l = session.lib_mut(ID)?;
    let s = score(&l.lib, id)?;
    let at = position(ID, p, "page", s)?;
    let tx = edit::set_page_hidden(&l.lib, id, at, hidden)?;
    l.commit(if hidden { "Hide Page" } else { "Show Page" }, tx)?;
    Ok(edited(l, json!({})))
}

fn page_delete(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "page.delete";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "pages"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let l = session.lib_mut(ID)?;
    let s = score(&l.lib, id)?;
    let n = s.pages.len() as u64;
    let pages = opt_int_list(ID, p, "pages", (1, n.max(1)), 100_000)?
        .ok_or_else(|| bad(ID, "`pages` is required".into()))?;
    if pages.is_empty() {
        return Err(bad(ID, "`pages` must name at least one page".into()));
    }
    let positions: Vec<usize> = pages.iter().map(|&x| (x - 1) as usize).collect();
    let tx = edit::delete_pages(&l.lib, id, &positions)?;
    l.commit(
        if positions.len() == 1 {
            "Delete Page"
        } else {
            "Delete Pages"
        },
        tx,
    )?;
    Ok(edited(
        l,
        json!({ "pages": l.lib.scores.get(&id).map(|s| s.pages.len()) }),
    ))
}

fn page_range_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "score.pageRange.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "first", "last"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let l = session.lib_mut(ID)?;
    let s = score(&l.lib, id)?;
    let n = s.pages.len() as u64;
    let first = opt_int(ID, p, "first", 1, n.max(1))?;
    let last = opt_int(ID, p, "last", 1, n.max(1))?;
    let range = match (first, last) {
        (None, None) => None,
        (f, l) => {
            let (f, l) = (f.unwrap_or(1), l.unwrap_or(n));
            Some(
                PageRange::new((f - 1) as u32, (l - 1) as u32)
                    .ok_or_else(|| bad(ID, "`first` must not be after `last`".into()))?,
            )
        }
    };
    l.commit(
        "Page Range",
        Transaction::single(Op::SetPageRange { score: id, range }),
    )?;
    Ok(edited(l, json!({})))
}

fn bookmark_add(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "score.bookmark.add";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "name", "page"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let name = req_str(ID, p, "name")?.trim().to_owned();
    check_len(ID, "name", &name, MAX_NAME)?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let s = score(&l.lib, id)?;
    let at = position(ID, p, "page", s)?;
    let page: PageKey = s.pages.get(at).map(|pg| pg.source).unwrap_or_default();
    let index = s.bookmarks.len();
    let bid: BookmarkId = l.ids.id(now);
    let name = if name.is_empty() {
        format!("Page {}", at + 1)
    } else {
        name
    };
    l.commit(
        "Add Bookmark",
        Transaction::single(Op::InsertBookmark {
            score: id,
            index,
            bookmark: Bookmark {
                id: bid,
                name,
                page,
            },
        }),
    )?;
    Ok(edited(l, json!({ "bookmark": bid.to_string() })))
}

fn bookmark_remove(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "score.bookmark.remove";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "bookmark"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let bookmark: BookmarkId = req_id(ID, p, "bookmark")?;
    let l = session.lib_mut(ID)?;
    l.commit(
        "Remove Bookmark",
        Transaction::single(Op::RemoveBookmark {
            score: id,
            bookmark,
        }),
    )?;
    Ok(edited(l, json!({})))
}
