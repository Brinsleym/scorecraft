//! Edits that need several ops to keep the library sound: emptying Recently Deleted, deleting a
//! folder, sending a folder to Recently Deleted and back, removing what has been there 30 days, deleting pages under a page range, removing a layer with markings on it. Each reads the
//! library and returns a [`Transaction`] to commit; none changes anything itself.

use scorecraft_doc::{EntryItem, FolderId, LayerId, Library, PageRange, ScoreId};
use scorecraft_geom::Rotation;

use crate::{Op, OpError, Transaction};

/// Permanently removes `score`: its entries in every list, its place in Recently Deleted, and
/// the score itself. Its blobs are left for the store's garbage collection.
pub fn remove_score(lib: &Library, score: ScoreId) -> Result<Transaction, OpError> {
    if !lib.scores.contains_key(&score) {
        return Err(OpError::NoScore(score));
    }
    let mut ops = Vec::new();
    for l in lib.lists.values() {
        for e in l.entries.iter().filter(|e| e.score() == Some(score)) {
            ops.push(Op::RemoveEntry {
                list: l.id,
                entry: e.id,
            });
        }
    }
    if lib.is_trashed(score) {
        ops.push(Op::Untrash { score });
    }
    ops.push(Op::RemoveScore { score });
    Ok(Transaction::new(ops))
}

/// Empties Recently Deleted (LIB-10): its scores and folders are removed for good.
pub fn empty_trash(lib: &Library) -> Result<Transaction, OpError> {
    purge_expired(lib, u64::MAX)
}

/// Deletes a folder, moving its scores and subfolders up to its parent (nothing is lost).
pub fn delete_folder(lib: &Library, folder: FolderId) -> Result<Transaction, OpError> {
    let f = lib.folders.get(&folder).ok_or(OpError::NoFolder(folder))?;
    let mut ops = Vec::new();
    for s in lib.scores.values().filter(|s| s.folder == Some(folder)) {
        ops.push(Op::SetScoreFolder {
            score: s.id,
            folder: f.parent,
        });
    }
    for child in lib.folders.values().filter(|c| c.parent == Some(folder)) {
        let mut moved = child.clone();
        moved.parent = f.parent;
        ops.push(Op::SetFolder { folder: moved });
    }
    ops.push(Op::RemoveFolder { folder });
    Ok(Transaction::new(ops))
}

/// How long Recently Deleted keeps a score or folder before it is removed for good (owner,
/// 2026-11-01).
pub const KEEP_DELETED_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// `folder` and every folder inside it, `folder` first. A corrupt tree with a cycle is walked
/// once.
pub fn folder_tree(lib: &Library, folder: FolderId) -> Vec<FolderId> {
    let mut out = vec![folder];
    let mut i = 0;
    while let Some(&f) = out.get(i) {
        for c in lib.folders.values().filter(|c| c.parent == Some(f)) {
            if !out.contains(&c.id) {
                out.push(c.id);
            }
        }
        i += 1;
    }
    out
}

/// Sends a folder to Recently Deleted (LIB-10): it, the folders inside it and every score in
/// them are marked deleted at `now`, keeping their places, so a restore puts everything back.
pub fn trash_folder(lib: &Library, folder: FolderId, now: u64) -> Result<Transaction, OpError> {
    let f = lib.folders.get(&folder).ok_or(OpError::NoFolder(folder))?;
    if f.deleted_ms.is_some() {
        return Err(OpError::Invalid(format!(
            "folder {} is already in Recently Deleted",
            f.name
        )));
    }
    let tree = folder_tree(lib, folder);
    let mut ops = Vec::new();
    for id in &tree {
        if let Some(f) = lib.folders.get(id).filter(|f| f.deleted_ms.is_none()) {
            let mut marked = f.clone();
            marked.deleted_ms = Some(now);
            ops.push(Op::SetFolder { folder: marked });
        }
    }
    for s in lib.scores.values() {
        if s.folder.is_some_and(|f| tree.contains(&f)) && !lib.is_trashed(s.id) {
            ops.push(Op::Trash {
                score: s.id,
                at_ms: now,
            });
        }
    }
    Ok(Transaction::new(ops))
}

/// Takes a folder out of Recently Deleted with what went with it: the folders and scores inside
/// it deleted at the same moment. Folders above it that are deleted come back too (only the
/// folders, so it can be seen); what else was in them stays deleted.
pub fn restore_folder(lib: &Library, folder: FolderId) -> Result<Transaction, OpError> {
    let f = lib.folders.get(&folder).ok_or(OpError::NoFolder(folder))?;
    let Some(at) = f.deleted_ms else {
        return Err(OpError::Invalid(format!(
            "folder {} is not in Recently Deleted",
            f.name
        )));
    };
    let tree = folder_tree(lib, folder);
    let mut ops = Vec::new();
    let restore = |f: &scorecraft_doc::Folder, ops: &mut Vec<Op>| {
        let mut back = f.clone();
        back.deleted_ms = None;
        ops.push(Op::SetFolder { folder: back });
    };
    for id in &tree {
        if let Some(f) = lib.folders.get(id).filter(|f| f.deleted_ms == Some(at)) {
            restore(f, &mut ops);
        }
    }
    for id in lib.ancestors(folder) {
        if let Some(f) = lib.folders.get(&id).filter(|f| f.deleted_ms.is_some()) {
            restore(f, &mut ops);
        }
    }
    for s in lib.scores.values() {
        if s.folder.is_some_and(|f| tree.contains(&f)) && lib.trash.get(&s.id) == Some(&at) {
            ops.push(Op::Untrash { score: s.id });
        }
    }
    Ok(Transaction::new(ops))
}

/// Removes for good what has been in Recently Deleted for [`KEEP_DELETED_MS`] or longer at
/// `now`: scores, then folders (anything still in a removed folder moves up to its parent, so
/// nothing is lost by accident).
pub fn purge_expired(lib: &Library, now: u64) -> Result<Transaction, OpError> {
    let expired = |at: u64| now.saturating_sub(at) >= KEEP_DELETED_MS;
    let mut scratch = lib.clone();
    let mut ops = Vec::new();
    let mut take = |tx: Transaction, scratch: &mut Library| -> Result<(), OpError> {
        for op in tx.ops {
            op.clone().apply(scratch)?;
            ops.push(op);
        }
        Ok(())
    };
    let scores: Vec<ScoreId> = lib
        .trash
        .iter()
        .filter(|(_, at)| expired(**at))
        .map(|(id, _)| *id)
        .collect();
    for id in scores {
        let tx = remove_score(&scratch, id)?;
        take(tx, &mut scratch)?;
    }
    let folders: Vec<FolderId> = lib
        .folders
        .values()
        .filter(|f| f.deleted_ms.is_some_and(expired))
        .map(|f| f.id)
        .collect();
    for id in folders {
        let tx = delete_folder(&scratch, id)?;
        take(tx, &mut scratch)?;
    }
    Ok(Transaction::new(ops))
}

/// Refuses a transaction that would change something in Recently Deleted, which is read-only
/// until restored (owner, 2026-11-01): any edit to a deleted score but removing it, and filing
/// into, renaming or moving a deleted folder. Sending things there and back is allowed.
pub fn check_not_deleted(lib: &Library, tx: &Transaction) -> Result<(), OpError> {
    let deleted_folder = |f: Option<FolderId>| f.is_some_and(|f| lib.folder_deleted(f));
    for op in &tx.ops {
        let refused = match op {
            Op::RemoveScore { .. } | Op::Trash { .. } | Op::Untrash { .. } => None,
            Op::SetScoreFolder { score, folder } => (lib.is_trashed(*score)
                || deleted_folder(*folder))
            .then(|| format!("score {score}")),
            Op::InsertFolder { folder } => {
                deleted_folder(folder.parent).then(|| format!("folder {}", folder.name))
            }
            Op::SetFolder { folder } => {
                let old = lib.folders.get(&folder.id);
                let editing_deleted = old.is_some_and(|o| {
                    o.deleted_ms.is_some()
                        && folder.deleted_ms.is_some()
                        && (o.name != folder.name || o.parent != folder.parent)
                });
                let into_deleted = folder.deleted_ms.is_none()
                    && parent_changed(old, folder)
                    && deleted_folder(folder.parent);
                (editing_deleted || into_deleted).then(|| format!("folder {}", folder.name))
            }
            _ => match op.touches() {
                crate::Touched::Score(id) if lib.is_trashed(id) => Some(format!("score {id}")),
                _ => None,
            },
        };
        if let Some(what) = refused {
            return Err(OpError::Invalid(format!(
                "{what} is in Recently Deleted: restore it to change it"
            )));
        }
    }
    Ok(())
}

fn parent_changed(old: Option<&scorecraft_doc::Folder>, new: &scorecraft_doc::Folder) -> bool {
    old.is_none_or(|o| o.parent != new.parent)
}

/// Deletes the pages at `positions` (into `Score::pages`) from the score (PAGE-4). The source
/// file and the pages' markings stay. A page range is shrunk to the pages that remain, or
/// cleared if none do.
pub fn delete_pages(
    lib: &Library,
    score: ScoreId,
    positions: &[usize],
) -> Result<Transaction, OpError> {
    let s = lib.scores.get(&score).ok_or(OpError::NoScore(score))?;
    let mut pos: Vec<usize> = positions.to_vec();
    pos.sort_unstable();
    pos.dedup();
    if let Some(&bad) = pos.iter().find(|&&p| p >= s.pages.len()) {
        return Err(OpError::OutOfRange {
            what: "page",
            index: bad,
            len: s.pages.len().saturating_sub(1),
        });
    }
    if pos.len() >= s.pages.len() {
        return Err(OpError::Invalid(
            "a score keeps at least one page: delete the score instead".into(),
        ));
    }
    let mut ops = Vec::new();
    let new_range = s.page_range.and_then(|r| shrink(r, &pos));
    if s.page_range.is_some() {
        ops.push(Op::SetPageRange { score, range: None });
    }
    for &p in pos.iter().rev() {
        ops.push(Op::RemovePage { score, index: p });
    }
    if new_range.is_some() {
        ops.push(Op::SetPageRange {
            score,
            range: new_range,
        });
    }
    // Setlist entries that play part of this score follow the same renumbering.
    for l in lib.lists.values() {
        for e in &l.entries {
            if let EntryItem::Score {
                score: sid,
                page_range: Some(r),
                tempo,
            } = e.item
                && sid == score
            {
                let shrunk = shrink(r, &pos);
                if shrunk != Some(r) {
                    let mut entry = e.clone();
                    entry.item = EntryItem::Score {
                        score,
                        page_range: shrunk,
                        tempo,
                    };
                    ops.push(Op::SetEntry { list: l.id, entry });
                }
            }
        }
    }
    Ok(Transaction::new(ops))
}

/// `r` after deleting the (sorted, deduplicated) page positions `deleted`: the surviving pages,
/// renumbered, or `None` if none survive.
fn shrink(r: PageRange, deleted: &[usize]) -> Option<PageRange> {
    let kept: Vec<u32> = (r.first..=r.last)
        .filter(|p| !deleted.contains(&(*p as usize)))
        .map(|p| p - deleted.iter().filter(|&&d| d < p as usize).count() as u32)
        .collect();
    PageRange::new(*kept.first()?, *kept.last()?)
}

/// Turns the page at `position` a further `by` clockwise (PAGE-2).
pub fn rotate_page(
    lib: &Library,
    score: ScoreId,
    position: usize,
    by: Rotation,
) -> Result<Transaction, OpError> {
    let s = lib.scores.get(&score).ok_or(OpError::NoScore(score))?;
    let mut page = *s.pages.get(position).ok_or(OpError::OutOfRange {
        what: "page",
        index: position,
        len: s.pages.len().saturating_sub(1),
    })?;
    page.rotation = page.rotation.then(by);
    Ok(Transaction::single(Op::SetPage {
        score,
        index: position,
        page,
    }))
}

/// Hides or shows the page at `position` (PAGE-3).
pub fn set_page_hidden(
    lib: &Library,
    score: ScoreId,
    position: usize,
    hidden: bool,
) -> Result<Transaction, OpError> {
    let s = lib.scores.get(&score).ok_or(OpError::NoScore(score))?;
    let mut page = *s.pages.get(position).ok_or(OpError::OutOfRange {
        what: "page",
        index: position,
        len: s.pages.len().saturating_sub(1),
    })?;
    if page.hidden == hidden {
        return Ok(Transaction::default());
    }
    if hidden && s.pages.iter().filter(|p| !p.hidden).count() <= 1 {
        return Err(OpError::Invalid(
            "a score keeps at least one visible page".into(),
        ));
    }
    page.hidden = hidden;
    Ok(Transaction::single(Op::SetPage {
        score,
        index: position,
        page,
    }))
}

/// Removes a layer and every annotation on it (ANNO-11).
pub fn remove_layer(lib: &Library, score: ScoreId, layer: LayerId) -> Result<Transaction, OpError> {
    let s = lib.scores.get(&score).ok_or(OpError::NoScore(score))?;
    if s.layer(layer).is_none() {
        return Err(OpError::Missing(format!("layer {layer}")));
    }
    let mut ops: Vec<Op> = s
        .annotations
        .iter()
        .flat_map(|(page, anns)| {
            anns.iter()
                .filter(|a| a.layer == layer)
                .map(|a| Op::RemoveAnnotation {
                    score,
                    page: *page,
                    annotation: a.id,
                })
        })
        .collect();
    ops.push(Op::RemoveLayer { score, layer });
    Ok(Transaction::new(ops))
}
