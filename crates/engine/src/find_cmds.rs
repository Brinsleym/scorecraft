//! Finding scores (spec `docs/spec/library.md`): search (LIB-5), sort (LIB-9), the recently
//! opened list (LIB-15) and importing a whole folder (LIB-2).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use scorecraft_doc::{Folder, FolderId, Library, Score, ScoreId};
use scorecraft_ops::{Op, Transaction};
use scorecraft_search::{Field, Query, SortKey, sort_key};

use crate::commands::{
    CommandSpec, always, bad, opt_bool, opt_id, opt_int, opt_str, params_object,
    reject_unknown_keys, req_str, truncate,
};
use crate::library::library_open;
use crate::library_cmds::{edited, import_one, summary};
use crate::{EngineError, Session};

/// The most results a search returns.
pub const MAX_RESULTS: usize = 500;
/// The recent list's default and largest length.
pub const RECENT_DEFAULT: u64 = 20;
pub const RECENT_MAX: u64 = 100;
/// Folder import limits.
pub const MAX_DEPTH: usize = 10;
pub const MAX_FOLDER_FILES: usize = 2000;
/// Folder chains are followed this far up (a cycle can't loop forever).
const MAX_FOLDER_CHAIN: usize = 64;

/// PDFs found under a folder: the folder names from the root to each file, and its path.
type Found = Vec<(Vec<String>, PathBuf)>;

/// How the library lists scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    #[default]
    Title,
    Composer,
    Added,
    Opened,
}

impl SortBy {
    pub fn id(self) -> &'static str {
        match self {
            SortBy::Title => "title",
            SortBy::Composer => "composer",
            SortBy::Added => "added",
            SortBy::Opened => "opened",
        }
    }

    fn from_id(s: &str) -> Option<SortBy> {
        [
            SortBy::Title,
            SortBy::Composer,
            SortBy::Added,
            SortBy::Opened,
        ]
        .into_iter()
        .find(|b| b.id() == s)
    }
}

/// The library's sort (session state; the shell keeps it between launches).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LibrarySort {
    pub by: SortBy,
    pub descending: bool,
}

impl LibrarySort {
    fn to_json(self) -> Value {
        json!({ "by": self.by.id(), "descending": self.descending })
    }
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
            "library.search",
            "Search",
            "",
            r#"{"query":string,"folder":id?,"trashed":bool=false,"limit":1..500=500}"#,
            false,
            library_open,
            search as _,
        ),
        spec(
            "library.sort.set",
            "Sort By",
            "View>Sort",
            r#"{"by":"title|composer|added|opened"?,"descending":bool?}"#,
            false,
            always,
            sort_set as _,
        ),
        spec(
            "library.recent.list",
            "Recently Opened",
            "File",
            r#"{"limit":1..100=20}"#,
            false,
            library_open,
            recent as _,
        ),
        spec(
            "library.import.folder",
            "Import Folder…",
            "File>Import",
            r#"{"path":path,"folder":id?,"subfolders":bool=true,"keepFolders":bool=false}"#,
            false,
            library_open,
            import_folder as _,
        ),
    ]
}

/// The names of `folder` and the folders above it.
fn folder_names(lib: &Library, mut folder: Option<FolderId>) -> Vec<&str> {
    let mut out = Vec::new();
    while let Some(id) = folder {
        let Some(f) = lib.folders.get(&id) else {
            break;
        };
        out.push(f.name.as_str());
        if out.len() >= MAX_FOLDER_CHAIN {
            break;
        }
        folder = f.parent;
    }
    out
}

/// Whether `score` is filed in `folder` or a folder inside it.
fn within(lib: &Library, score: &Score, folder: FolderId) -> bool {
    let mut at = score.folder;
    for _ in 0..MAX_FOLDER_CHAIN {
        match at {
            Some(f) if f == folder => return true,
            Some(f) => at = lib.folders.get(&f).and_then(|x| x.parent),
            None => return false,
        }
    }
    false
}

/// Orders `scores` by `sort` (ties by title, then id, so the order is stable).
pub(crate) fn sorted(sort: LibrarySort, mut scores: Vec<&Score>) -> Vec<&Score> {
    let title = |s: &Score| sort_key(s.display_title());
    let key = |s: &Score| -> (u8, u64, SortKey, SortKey) {
        match sort.by {
            SortBy::Title => (0, 0, title(s), SortKey::default()),
            SortBy::Composer => (0, 0, sort_key(&s.metadata.composer), title(s)),
            // Newest first: compare the negated time.
            SortBy::Added => (0, u64::MAX - s.added_ms, title(s), SortKey::default()),
            SortBy::Opened => match s.opened_ms {
                Some(t) => (0, u64::MAX - t, title(s), SortKey::default()),
                None => (1, 0, title(s), SortKey::default()),
            },
        }
    };
    let mut keyed: Vec<_> = scores.drain(..).map(|s| (key(s), s.id, s)).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    if sort.descending {
        keyed.reverse();
    }
    keyed.into_iter().map(|(_, _, s)| s).collect()
}

fn search(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.search";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["query", "folder", "trashed", "limit"])?;
    let text = req_str(ID, p, "query")?;
    let query = Query::parse(text).map_err(|e| bad(ID, e.to_string()))?;
    let folder: Option<FolderId> = opt_id(ID, p, "folder")?;
    let trashed = opt_bool(ID, p, "trashed")?.unwrap_or(false);
    let limit = opt_int(ID, p, "limit", 1, MAX_RESULTS as u64)?.unwrap_or(MAX_RESULTS as u64);
    let sort = session.library_sort;
    let l = session.lib_mut(ID)?;
    let lib = &l.lib;
    if let Some(f) = folder
        && !lib.folders.contains_key(&f)
    {
        return Err(scorecraft_ops::OpError::NoFolder(f).into());
    }
    let pool: Vec<&Score> = lib
        .scores
        .values()
        .filter(|s| trashed || !lib.is_trashed(s.id))
        .filter(|s| folder.is_none_or(|f| within(lib, s, f)))
        .collect();
    let ordered = sorted(sort, pool);
    let mut hits: Vec<(u32, usize, &Score)> = ordered
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let tags = s.metadata.tags.join(" ");
            let folders = folder_names(lib, s.folder).join(" ");
            let fields = [
                (Field::Title, s.metadata.title.as_str()),
                (Field::Composer, s.metadata.composer.as_str()),
                (Field::Other, s.metadata.arranger.as_str()),
                (Field::Other, s.metadata.key.as_str()),
                (Field::Other, s.metadata.genre.as_str()),
                (Field::Other, tags.as_str()),
                (Field::Other, s.source.file_name.as_str()),
                (Field::Other, folders.as_str()),
            ];
            query.rank(&fields).map(|r| (r, i, s))
        })
        .collect();
    // Best match first; ties keep the sort order.
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let count = hits.len();
    let list: Vec<Value> = hits
        .into_iter()
        .take(usize::try_from(limit).unwrap_or(MAX_RESULTS))
        .map(|(rank, _, s)| {
            let mut v = summary(lib, s);
            crate::commands::set(&mut v, "rank", json!(rank));
            v
        })
        .collect();
    Ok(json!({ "count": count, "scores": list, "sort": sort.to_json() }))
}

fn sort_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.sort.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["by", "descending"])?;
    if let Some(by) = opt_str(ID, p, "by")? {
        session.library_sort.by = SortBy::from_id(by).ok_or_else(|| {
            bad(
                ID,
                format!(
                    "`by` must be title, composer, added or opened, got {}",
                    truncate(by, 32)
                ),
            )
        })?;
    }
    if let Some(d) = opt_bool(ID, p, "descending")? {
        session.library_sort.descending = d;
    }
    Ok(json!({ "sort": session.library_sort.to_json() }))
}

fn recent(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.recent.list";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["limit"])?;
    let limit = opt_int(ID, p, "limit", 1, RECENT_MAX)?.unwrap_or(RECENT_DEFAULT);
    let l = session.lib_mut(ID)?;
    let lib = &l.lib;
    let opened: Vec<&Score> = lib
        .scores
        .values()
        .filter(|s| s.opened_ms.is_some() && !lib.is_trashed(s.id))
        .collect();
    let list: Vec<Value> = sorted(
        LibrarySort {
            by: SortBy::Opened,
            descending: false,
        },
        opened,
    )
    .into_iter()
    .take(usize::try_from(limit).unwrap_or(20))
    .map(|s| summary(lib, s))
    .collect();
    Ok(json!({ "scores": list }))
}

/// The PDFs under `root`, folder by folder (paths relative to `root`), at most
/// [`MAX_FOLDER_FILES`] and [`MAX_DEPTH`] levels down. Symbolic links are not followed.
fn pdfs_under(root: &Path, subfolders: bool) -> Result<(Found, usize), EngineError> {
    let mut out = Vec::new();
    let mut ignored = 0;
    let mut stack = vec![(root.to_path_buf(), Vec::<String>::new())];
    while let Some((dir, rel)) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .map_err(|e| {
                EngineError::Io(format!("{}: {e}", truncate(&dir.to_string_lossy(), 256)))
            })?
            .filter_map(Result::ok)
            .collect();
        entries.sort_by_key(|e| e.file_name());
        let mut subdirs = Vec::new();
        for e in entries {
            let Ok(kind) = e.file_type() else { continue };
            let path = e.path();
            if kind.is_dir() {
                if subfolders && rel.len() < MAX_DEPTH {
                    let mut r = rel.clone();
                    r.push(e.file_name().to_string_lossy().into_owned());
                    subdirs.push((path, r));
                }
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
            {
                if out.len() >= MAX_FOLDER_FILES {
                    return Err(bad(
                        "library.import.folder",
                        format!(
                            "the folder holds more than {MAX_FOLDER_FILES} PDFs; import part of it"
                        ),
                    ));
                }
                out.push((rel.clone(), path));
            } else if kind.is_file() {
                ignored += 1;
            }
        }
        // Visit subfolders in name order.
        stack.extend(subdirs.into_iter().rev());
    }
    Ok((out, ignored))
}

fn import_folder(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.import.folder";
    let p: Option<&Map<String, Value>> = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["path", "folder", "subfolders", "keepFolders"])?;
    let path = req_str(ID, p, "path")?;
    let target: Option<FolderId> = opt_id(ID, p, "folder")?;
    let subfolders = opt_bool(ID, p, "subfolders")?.unwrap_or(true);
    let keep = opt_bool(ID, p, "keepFolders")?.unwrap_or(false);
    let root = Path::new(path);
    if !root.is_dir() {
        return Err(EngineError::Io(format!(
            "{} is not a folder",
            truncate(path, 256)
        )));
    }
    let (files, ignored) = pdfs_under(root, subfolders)?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    if let Some(f) = target
        && !l.lib.folders.contains_key(&f)
    {
        return Err(scorecraft_ops::OpError::NoFolder(f).into());
    }
    let mut known: HashSet<_> = l
        .lib
        .scores
        .values()
        .filter(|s| !l.lib.is_trashed(s.id))
        .map(|s| s.source.blob)
        .collect();
    let mut ops = Vec::new();
    let mut folders: BTreeMap<Vec<String>, FolderId> = BTreeMap::new();
    let (mut imported, mut skipped, mut failed) = (Vec::<ScoreId>::new(), 0usize, Vec::new());
    for (rel, file) in files {
        let shown = truncate(&file.to_string_lossy(), 256);
        let bytes = match crate::score_cmds::read_score(&file.to_string_lossy()) {
            Ok(b) => b,
            Err(e) => {
                failed.push(json!({ "path": shown, "code": e.code(), "message": e.to_string() }));
                continue;
            }
        };
        let blob = l.store.put_blob(&bytes)?;
        if !known.insert(blob) {
            skipped += 1;
            continue;
        }
        // The library folder for this file's folder, made on first use.
        let mut parent = target;
        if keep {
            for depth in 1..=rel.len() {
                let key = rel.get(..depth).map(<[String]>::to_vec).unwrap_or_default();
                let id = match folders.get(&key) {
                    Some(id) => *id,
                    None => {
                        let id: FolderId = l.ids.id(now);
                        let name = key.last().cloned().unwrap_or_default();
                        ops.push(Op::InsertFolder {
                            folder: Folder { id, name, parent },
                        });
                        folders.insert(key, id);
                        id
                    }
                };
                parent = Some(id);
            }
        }
        match import_one(l, &file.to_string_lossy(), parent, now) {
            Ok(score) => {
                imported.push(score.id);
                ops.push(Op::InsertScore {
                    score: Box::new(score),
                });
            }
            Err(e) => {
                failed.push(json!({ "path": shown, "code": e.code(), "message": e.to_string() }))
            }
        }
    }
    if !ops.is_empty() {
        l.commit("Import Folder", Transaction::new(ops))?;
    }
    Ok(edited(
        l,
        json!({
            "imported": imported.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "skipped": skipped,
            "ignored": ignored,
            "folders": folders.len(),
            "failed": failed,
        }),
    ))
}

#[cfg(test)]
mod tests;
