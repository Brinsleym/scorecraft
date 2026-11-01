//! Full-text search (feature-map LIB-6, spec `docs/spec/library.md`): the words inside every
//! score's PDF and its practice notes. Page text is extracted once per file and kept in the
//! library's cache (`cache/text/<blob>.txt`); a search indexes what it hasn't seen yet within a
//! time budget and says how much is still waiting, so the next search finds more.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value, json};

use scorecraft_doc::Score;
use scorecraft_search::{Query, fold, words};

use crate::commands::{CommandSpec, bad, opt_int, params_object, reject_unknown_keys, req_str};
use crate::library::library_open;
use crate::library_cmds::summary;
use crate::{EngineError, Session};

/// Time spent extracting text in one search, in ms.
const INDEX_BUDGET_MS: u64 = 1500;
/// Text kept per page and per score, in characters.
const MAX_PAGE_CHARS: usize = 100_000;
const MAX_SCORE_CHARS: usize = 2_000_000;
/// Pages are joined with a form feed in the cache.
const PAGE_BREAK: char = '\u{c}';
/// The most results.
const MAX_RESULTS: u64 = 200;

/// One file's text: its pages, and its words folded, sorted and deduplicated for matching.
#[derive(Debug)]
pub struct TextIndex {
    pages: Vec<String>,
    words: Vec<String>,
}

impl TextIndex {
    fn new(pages: Vec<String>) -> TextIndex {
        let mut w: Vec<String> = pages.iter().flat_map(|p| words(p)).collect();
        w.sort_unstable();
        w.dedup();
        TextIndex { pages, words: w }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: "library.search.fullText",
        label: "Search Inside Scores",
        menu: "",
        shortcut: None,
        params: r#"{"query":string,"limit":1..200=50,"budgetMs":0..2000=1500}"#,
        ours: false,
        enabled: library_open,
        run: search,
    }]
}

/// The text of `score`'s file: from memory, from the library's cache, or extracted now when
/// `may_extract`. `None` when it isn't indexed yet.
fn text_of(
    session: &mut Session,
    score: &Score,
    may_extract: bool,
) -> Result<Option<Arc<TextIndex>>, EngineError> {
    let key = score.source.blob.to_hex();
    if let Some(t) = session.text_index.get(&key) {
        return Ok(Some(t.clone()));
    }
    let name = format!("text/{key}.txt");
    let l = session.lib_mut("library.search.fullText")?;
    let cached = l.store.cache(&name)?;
    let pages: Vec<String> = match cached {
        Some(bytes) => String::from_utf8_lossy(&bytes)
            .split(PAGE_BREAK)
            .map(str::to_owned)
            .collect(),
        None if may_extract => {
            let bytes = Arc::new(l.store.blob(score.source.blob)?);
            let pages = match scorecraft_pdf::Document::open(bytes) {
                Ok(mut doc) => {
                    let mut total = 0;
                    let mut pages = Vec::new();
                    for i in 0..doc.page_count() {
                        let t: String = doc
                            .text(i)
                            .map(|t| t.plain)
                            .unwrap_or_default()
                            .chars()
                            .filter(|c| *c != PAGE_BREAK)
                            .take(MAX_PAGE_CHARS.min(MAX_SCORE_CHARS.saturating_sub(total)))
                            .collect();
                        total += t.chars().count();
                        pages.push(t);
                    }
                    pages
                }
                // A file that can't be read has no text; it is still searched by its notes.
                Err(_) => Vec::new(),
            };
            let joined: String = pages.join(&PAGE_BREAK.to_string());
            l.store.set_cache(&name, joined.as_bytes())?;
            pages
        }
        None => return Ok(None),
    };
    let idx = Arc::new(TextIndex::new(pages));
    session.text_index.insert(key, idx.clone());
    Ok(Some(idx))
}

/// About 100 characters of `text` around the first word that starts with `q` (folded).
fn snippet(text: &str, q: &str) -> Option<String> {
    let mut at = None;
    let mut pos = 0usize;
    for w in text.split(|c: char| !c.is_alphanumeric()) {
        if !w.is_empty() && fold(w).starts_with(q) {
            at = Some(pos);
            break;
        }
        pos += w.len() + 1;
    }
    let at = at?;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let i = chars.iter().position(|(b, _)| *b >= at).unwrap_or(0);
    let from = i.saturating_sub(40);
    let to = (i + 60).min(chars.len());
    let s: String = chars
        .get(from..to)
        .unwrap_or(&[])
        .iter()
        .map(|(_, c)| if c.is_whitespace() { ' ' } else { *c })
        .collect();
    Some(format!(
        "{}{}{}",
        if from > 0 { "…" } else { "" },
        s.trim(),
        if to < chars.len() { "…" } else { "" }
    ))
}

fn search(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.search.fullText";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["query", "limit", "budgetMs"])?;
    // How long this search may spend indexing files it hasn't seen (the app asks for a little
    // each frame so the window never stalls).
    let budget = opt_int(ID, p, "budgetMs", 0, 2000)?.unwrap_or(INDEX_BUDGET_MS);
    let text = req_str(ID, p, "query")?;
    let query = Query::parse(text).map_err(|e| bad(ID, e.to_string()))?;
    if query.is_empty() {
        return Err(bad(ID, "give some words to search for".into()));
    }
    let limit = opt_int(ID, p, "limit", 1, MAX_RESULTS)?.unwrap_or(50);
    let scores: Vec<Score> = {
        let l = session.lib_mut(ID)?;
        l.lib
            .scores
            .values()
            .filter(|s| !l.lib.is_trashed(s.id))
            .cloned()
            .collect()
    };
    let started = session.now_ms();
    let (mut hits, mut pending) = (Vec::new(), 0usize);
    for s in &scores {
        let budget_left = session.now_ms().saturating_sub(started) < budget;
        let idx = text_of(session, s, budget_left)?;
        if idx.is_none() {
            pending += 1;
        }
        let notes = format!("{} {}", s.practice_notes, s.metadata.notes);
        let mut note_words = words(&notes);
        note_words.sort_unstable();
        note_words.dedup();
        let in_text = idx.as_ref().is_some_and(|i| query.matches_sorted(&i.words));
        let in_notes = query.matches_sorted(&note_words);
        if !(in_text || in_notes) {
            continue;
        }
        let first = query.words().first().cloned().unwrap_or_default();
        let found = idx.as_ref().filter(|_| in_text).and_then(|i| {
            i.pages
                .iter()
                .enumerate()
                .find_map(|(n, t)| snippet(t, &first).map(|sn| (n, sn)))
        });
        hits.push((
            s.id,
            found,
            in_notes.then(|| snippet(&notes, &first)).flatten(),
        ));
    }
    let count = hits.len();
    let l = session.lib_mut(ID)?;
    let list: Vec<Value> = hits
        .into_iter()
        .take(usize::try_from(limit).unwrap_or(50))
        .filter_map(|(id, found, note)| {
            let s = l.lib.scores.get(&id)?;
            let mut v = summary(&l.lib, s);
            if let Some((page, sn)) = found {
                crate::commands::set(&mut v, "filePage", json!(page + 1));
                crate::commands::set(&mut v, "snippet", json!(sn));
            }
            if let Some(n) = note {
                crate::commands::set(&mut v, "notes", json!(n));
            }
            Some(v)
        })
        .collect();
    Ok(json!({ "count": count, "scores": list, "pending": pending }))
}

/// The session's in-memory text indexes, by file.
pub(crate) type TextIndexes = HashMap<String, Arc<TextIndex>>;

#[cfg(test)]
mod tests;
