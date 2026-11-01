//! The library: create and open one, import scores, folders, Recently Deleted, Verify & Repair,
//! and read-only views of its state for agents (feature-map LIB-1, LIB-3, LIB-10, LIB-12).

use std::sync::Arc;

use serde_json::{Value, json};

use scorecraft_doc::{Folder, FolderId, Library, Metadata, Score, ScoreId, Source, SourceKind};
use scorecraft_ops::{Op, Transaction, edit};

use crate::commands::{
    CommandSpec, always, bad, check_len, opt_bool, opt_id, opt_str, opt_str_list, params_object,
    reject_unknown_keys, req_id, req_str,
};
use crate::library::library_open;
use crate::{EngineError, Session};

/// Most files one `library.import.files` call takes.
pub const MAX_IMPORT_FILES: usize = 1000;
/// Longest name or title accepted.
pub const MAX_NAME: usize = 500;

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
            "library.create",
            "New Library…",
            "File",
            r#"{"path":path}"#,
            true,
            always,
            create as _,
        ),
        spec(
            "library.open",
            "Open Library…",
            "File",
            r#"{"path":path}"#,
            true,
            always,
            open as _,
        ),
        spec(
            "library.close",
            "Close Library",
            "File",
            "{}",
            true,
            library_open,
            close as _,
        ),
        spec(
            "library.info",
            "Library Info",
            "",
            "{}",
            true,
            library_open,
            info as _,
        ),
        spec(
            "library.scores",
            "List Scores",
            "",
            r#"{"folder":id|"root"?,"trashed":bool=false}"#,
            true,
            library_open,
            scores as _,
        ),
        spec(
            "library.score.get",
            "Score Details",
            "",
            r#"{"score":id}"#,
            true,
            library_open,
            score_get as _,
        ),
        spec(
            "library.import.files",
            "Import Files…",
            "File>Import",
            r#"{"paths":[path],"folder":id?}"#,
            false,
            library_open,
            import_files as _,
        ),
        spec(
            "library.openFile",
            "Open Score…",
            "File",
            r#"{"path":path,"password":string?}"#,
            true,
            library_open,
            open_file as _,
        ),
        spec(
            "library.folder.new",
            "New Folder",
            "Library",
            r#"{"name":string,"parent":id?}"#,
            false,
            library_open,
            folder_new as _,
        ),
        spec(
            "library.folder.rename",
            "Rename Folder",
            "Library",
            r#"{"folder":id,"name":string}"#,
            false,
            library_open,
            folder_rename as _,
        ),
        spec(
            "library.folder.delete",
            "Delete Folder",
            "Library",
            r#"{"folder":id}"#,
            false,
            library_open,
            folder_delete as _,
        ),
        spec(
            "library.score.move",
            "Move to Folder…",
            "Library",
            r#"{"score":id,"folder":id|null}"#,
            false,
            library_open,
            score_move as _,
        ),
        spec(
            "library.score.delete",
            "Delete Score",
            "Library",
            r#"{"score":id}"#,
            false,
            library_open,
            score_delete as _,
        ),
        spec(
            "library.trash.restore",
            "Restore",
            "Library>Recently Deleted",
            r#"{"score":id}"#,
            false,
            library_open,
            trash_restore as _,
        ),
        spec(
            "library.trash.empty",
            "Empty Recently Deleted",
            "Library>Recently Deleted",
            "{}",
            false,
            library_open,
            trash_empty as _,
        ),
        spec(
            "library.verify",
            "Verify & Repair Library",
            "Library",
            r#"{"repair":bool=false}"#,
            false,
            library_open,
            verify as _,
        ),
    ]
}

fn create(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.create";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["path"])?;
    let path = req_str(ID, p, "path")?;
    session.create_library_dir(path)?;
    info(session, &Value::Null)
}

fn open(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.open";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["path"])?;
    let path = req_str(ID, p, "path")?;
    let skipped = session.open_library_dir(path)?;
    let mut v = info(session, &Value::Null)?;
    crate::commands::set(
        &mut v,
        "unreadable",
        skipped
            .iter()
            .map(|(p, why)| json!({ "path": p, "reason": why }))
            .collect(),
    );
    Ok(v)
}

fn close(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.close";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    session.library = None;
    Ok(json!({ "closed": true }))
}

fn info(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.info";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    let l = session.lib_mut(ID)?;
    Ok(json!({
        "id": l.lib.id.to_string(),
        "location": l.location,
        "scores": l.lib.live_scores().count(),
        "trashed": l.lib.trash.len(),
        "folders": l.lib.folders.values().map(|f| json!({
            "id": f.id.to_string(),
            "name": f.name,
            "parent": f.parent.map(|p| p.to_string()),
            "path": l.lib.folder_path(f.id),
        })).collect::<Vec<_>>(),
        "lists": l.lib.lists.values().map(|x| json!({
            "id": x.id.to_string(),
            "name": x.name,
            "entries": x.entries.len(),
        })).collect::<Vec<_>>(),
        "canUndo": l.history.undo_label(None),
        "canRedo": l.history.redo_label(None),
    }))
}

pub(crate) fn summary(lib: &Library, s: &Score) -> Value {
    json!({
        "id": s.id.to_string(),
        "title": s.display_title(),
        "composer": s.metadata.composer,
        "tags": s.metadata.tags,
        "folder": s.folder.map(|f| f.to_string()),
        "pages": s.pages.len(),
        "visiblePages": s.visible_pages().len(),
        "trashed": lib.is_trashed(s.id),
        "addedMs": s.added_ms,
        "openedMs": s.opened_ms,
    })
}

fn scores(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.scores";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["folder", "trashed"])?;
    let folder = match opt_str(ID, p, "folder")? {
        None => None,
        Some("root") => Some(None),
        Some(s) => Some(Some(
            s.parse::<FolderId>()
                .map_err(|e| bad(ID, format!("`folder`: {e}")))?,
        )),
    };
    let trashed = opt_bool(ID, p, "trashed")?.unwrap_or(false);
    let sort = session.library_sort;
    let l = session.lib_mut(ID)?;
    let lib = &l.lib;
    let pool: Vec<&Score> = lib
        .scores
        .values()
        .filter(|s| lib.is_trashed(s.id) == trashed)
        .filter(|s| folder.is_none_or(|f| s.folder == f))
        .collect();
    let list: Vec<Value> = crate::find_cmds::sorted(sort, pool)
        .into_iter()
        .map(|s| summary(lib, s))
        .collect();
    Ok(json!({ "scores": list }))
}

fn score_get(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.score.get";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let l = session.lib_mut(ID)?;
    let s = l
        .lib
        .scores
        .get(&id)
        .ok_or(scorecraft_ops::OpError::NoScore(id))?;
    let mut v = serde_json::to_value(s).map_err(|e| EngineError::Io(e.to_string()))?;
    crate::commands::set(&mut v, "trashed", json!(l.lib.is_trashed(id)));
    crate::commands::set(&mut v, "visiblePages", json!(s.visible_pages()));
    Ok(v)
}

/// The result of one undoable edit, for every edit command.
pub(crate) fn edited(l: &crate::library::LibrarySession, extra: Value) -> Value {
    let mut v = json!({
        "undo": l.history.undo_label(None),
    });
    if let (Some(o), Value::Object(e)) = (v.as_object_mut(), extra) {
        o.extend(e);
    }
    v
}

fn import_files(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.import.files";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["paths", "folder"])?;
    let paths = opt_str_list(ID, p, "paths", MAX_IMPORT_FILES)?
        .ok_or_else(|| bad(ID, "`paths` is required".into()))?;
    let folder: Option<FolderId> = opt_id(ID, p, "folder")?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    if let Some(f) = folder
        && !l.lib.folders.contains_key(&f)
    {
        return Err(scorecraft_ops::OpError::NoFolder(f).into());
    }
    let mut ops = Vec::new();
    let mut imported = Vec::new();
    let mut failed = Vec::new();
    for path in &paths {
        let shown = crate::commands::truncate(path, 256);
        match import_one(l, path, folder, now) {
            Ok(score) => {
                imported.push(json!({
                    "path": shown,
                    "score": score.id.to_string(),
                    "title": score.display_title(),
                    "pages": score.pages.len(),
                }));
                ops.push(Op::InsertScore {
                    score: Box::new(score),
                });
            }
            Err(e) => {
                failed.push(json!({ "path": shown, "code": e.code(), "message": e.to_string() }))
            }
        }
    }
    let label = if ops.len() == 1 {
        "Import Score".to_owned()
    } else {
        format!("Import {} Scores", ops.len())
    };
    l.commit(&label, Transaction::new(ops))?;
    Ok(edited(l, json!({ "imported": imported, "failed": failed })))
}

/// Opens a score file through the library: the score already holding the same file (by its
/// content) if there is one in the library and not in the trash, else a new import. What the app
/// does with a file the player picks or drops, so markings stay with the score.
fn open_file(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.openFile";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["path", "password"])?;
    let path = req_str(ID, p, "path")?;
    let password = opt_str(ID, p, "password")?;
    let bytes = crate::score_cmds::read_score(path)?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let blob = l.store.put_blob(&bytes)?;
    let existing = l
        .lib
        .scores
        .values()
        .find(|s| s.source.blob == blob && !l.lib.is_trashed(s.id))
        .map(|s| s.id);
    let (id, imported) = match existing {
        Some(id) => (id, false),
        None => {
            let score = import_one(l, path, None, now)?;
            let id = score.id;
            l.commit(
                "Import Score",
                Transaction::single(Op::InsertScore {
                    score: Box::new(score),
                }),
            )?;
            (id, true)
        }
    };
    let mut v = session.open_library_score(id, password)?;
    crate::commands::set(&mut v, "score", json!(id.to_string()));
    crate::commands::set(&mut v, "imported", json!(imported));
    Ok(v)
}

pub(crate) fn import_one(
    l: &mut crate::library::LibrarySession,
    path: &str,
    folder: Option<FolderId>,
    now: u64,
) -> Result<Score, EngineError> {
    let bytes = crate::score_cmds::read_score(path)?;
    let bytes = Arc::new(bytes);
    // Opening it checks it is a PDF we can read before anything is stored.
    let doc = scorecraft_pdf::Document::open(bytes.clone())?;
    let page_count = u32::try_from(doc.page_count())
        .map_err(|_| EngineError::Unsupported("more than 4 billion pages".into()))?;
    let m = doc.metadata();
    let file_name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = std::path::Path::new(&file_name)
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let blob = l.store.put_blob(&bytes)?;
    let id = l.ids.id(now);
    let layer = l.ids.id(now);
    let mut score = Score::new(
        id,
        Source {
            kind: SourceKind::Pdf,
            blob,
            more: Vec::new(),
            file_name,
            page_count,
        },
        layer,
        now,
    );
    let clean = |s: &Option<String>| {
        s.as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| t.chars().take(MAX_NAME).collect::<String>())
    };
    score.metadata = Metadata {
        title: clean(&m.title).unwrap_or(stem),
        composer: clean(&m.author).unwrap_or_default(),
        ..Metadata::default()
    };
    score.folder = folder;
    Ok(score)
}

fn folder_new(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.folder.new";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["name", "parent"])?;
    let name = req_str(ID, p, "name")?.trim().to_owned();
    if name.is_empty() {
        return Err(bad(ID, "`name` must not be empty".into()));
    }
    check_len(ID, "name", &name, MAX_NAME)?;
    let parent: Option<FolderId> = opt_id(ID, p, "parent")?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let id: FolderId = l.ids.id(now);
    l.commit(
        "New Folder",
        Transaction::single(Op::InsertFolder {
            folder: Folder { id, name, parent },
        }),
    )?;
    Ok(edited(l, json!({ "folder": id.to_string() })))
}

fn folder_rename(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.folder.rename";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["folder", "name"])?;
    let id: FolderId = req_id(ID, p, "folder")?;
    let name = req_str(ID, p, "name")?.trim().to_owned();
    if name.is_empty() {
        return Err(bad(ID, "`name` must not be empty".into()));
    }
    check_len(ID, "name", &name, MAX_NAME)?;
    let l = session.lib_mut(ID)?;
    let mut f = l
        .lib
        .folders
        .get(&id)
        .cloned()
        .ok_or(scorecraft_ops::OpError::NoFolder(id))?;
    f.name = name;
    l.commit(
        "Rename Folder",
        Transaction::single(Op::SetFolder { folder: f }),
    )?;
    Ok(edited(l, json!({ "folder": id.to_string() })))
}

fn folder_delete(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.folder.delete";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["folder"])?;
    let id: FolderId = req_id(ID, p, "folder")?;
    let l = session.lib_mut(ID)?;
    let tx = edit::delete_folder(&l.lib, id)?;
    l.commit("Delete Folder", tx)?;
    Ok(edited(l, json!({})))
}

fn score_move(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.score.move";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "folder"])?;
    let score: ScoreId = req_id(ID, p, "score")?;
    let folder: Option<FolderId> = opt_id(ID, p, "folder")?;
    let l = session.lib_mut(ID)?;
    l.commit(
        "Move to Folder",
        Transaction::single(Op::SetScoreFolder { score, folder }),
    )?;
    Ok(edited(l, json!({})))
}

fn score_delete(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.score.delete";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score"])?;
    let score: ScoreId = req_id(ID, p, "score")?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    l.commit(
        "Delete Score",
        Transaction::single(Op::Trash { score, at_ms: now }),
    )?;
    Ok(edited(
        l,
        json!({ "lists": l.lib.lists_using(score).iter().map(|i| i.to_string()).collect::<Vec<_>>() }),
    ))
}

fn trash_restore(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.trash.restore";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score"])?;
    let score: ScoreId = req_id(ID, p, "score")?;
    let l = session.lib_mut(ID)?;
    l.commit("Restore Score", Transaction::single(Op::Untrash { score }))?;
    Ok(edited(l, json!({})))
}

fn trash_empty(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.trash.empty";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    let l = session.lib_mut(ID)?;
    let count = l.lib.trash.len();
    let tx = edit::empty_trash(&l.lib)?;
    l.commit("Empty Recently Deleted", tx)?;
    Ok(edited(l, json!({ "removed": count })))
}

fn verify(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.verify";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["repair"])?;
    let repair = opt_bool(ID, p, "repair")?.unwrap_or(false);
    let l = session.lib_mut(ID)?;
    let report = l.store.verify(&l.lib, &l.unreadable)?;
    let hex = |v: &[scorecraft_doc::BlobRef]| v.iter().map(|b| b.to_hex()).collect::<Vec<_>>();
    let mut out = json!({
        "clean": report.is_clean(),
        "problems": report.problems,
        "missingFiles": hex(&report.missing_blobs),
        "damagedFiles": hex(&report.corrupt_blobs),
        "unusedFiles": hex(&report.unused_blobs),
        "unreadable": report.unreadable,
    });
    if repair {
        let unreadable = std::mem::take(&mut l.unreadable);
        let r = l.store.repair(&mut l.lib, &unreadable)?;
        let removed = l.store.collect_garbage(&l.lib)?;
        // Repair rewrites records outside the undo history; steps recorded before it may no
        // longer apply.
        l.history.clear();
        crate::commands::set(
            &mut out,
            "repaired",
            json!({
                "fixes": r.fixes,
                "quarantined": r.quarantined,
                "unusedFilesRemoved": removed,
            }),
        );
    }
    Ok(out)
}
