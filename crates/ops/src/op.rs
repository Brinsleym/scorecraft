//! Reversible operations on a [`Library`]. Each op checks everything it needs before changing
//! anything, so a failed op leaves the library untouched, and a successful one returns the op
//! that undoes it.

use serde::{Deserialize, Serialize};

use scorecraft_doc::{
    Annotation, AnnotationId, AudioTrack, Bookmark, BookmarkId, EntryId, Folder, FolderId, Layer,
    LayerId, Library, ListEntry, ListId, Metadata, MidiOnOpen, PageKey, PageRange, PageRef, Score,
    ScoreId, Setlist,
};

use crate::OpError;

/// One reversible change. Serialisable, so an op log can be written and replayed (sync, M8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Op {
    // ---- scores
    InsertScore {
        score: Box<Score>,
    },
    RemoveScore {
        score: ScoreId,
    },
    SetMetadata {
        score: ScoreId,
        metadata: Box<Metadata>,
    },
    SetScoreFolder {
        score: ScoreId,
        folder: Option<FolderId>,
    },
    SetPageRange {
        score: ScoreId,
        range: Option<PageRange>,
    },
    SetAudio {
        score: ScoreId,
        audio: Option<Box<AudioTrack>>,
    },
    SetMidiOnOpen {
        score: ScoreId,
        midi: Option<MidiOnOpen>,
    },
    SetPracticeNotes {
        score: ScoreId,
        notes: String,
    },
    // ---- pages (positions into `Score::pages`)
    InsertPage {
        score: ScoreId,
        index: usize,
        page: PageRef,
    },
    RemovePage {
        score: ScoreId,
        index: usize,
    },
    SetPage {
        score: ScoreId,
        index: usize,
        page: PageRef,
    },
    MovePage {
        score: ScoreId,
        from: usize,
        to: usize,
    },
    // ---- layers
    InsertLayer {
        score: ScoreId,
        index: usize,
        layer: Layer,
    },
    RemoveLayer {
        score: ScoreId,
        layer: LayerId,
    },
    SetLayer {
        score: ScoreId,
        layer: Layer,
    },
    /// Moves the layer at `from` to `to` in drawing order (ANNO-10).
    MoveLayer {
        score: ScoreId,
        from: usize,
        to: usize,
    },
    // ---- annotations (on source page `page`)
    InsertAnnotation {
        score: ScoreId,
        page: PageKey,
        index: usize,
        annotation: Box<Annotation>,
    },
    RemoveAnnotation {
        score: ScoreId,
        page: PageKey,
        annotation: AnnotationId,
    },
    SetAnnotation {
        score: ScoreId,
        page: PageKey,
        annotation: Box<Annotation>,
    },
    // ---- bookmarks
    InsertBookmark {
        score: ScoreId,
        index: usize,
        bookmark: Bookmark,
    },
    RemoveBookmark {
        score: ScoreId,
        bookmark: BookmarkId,
    },
    SetBookmark {
        score: ScoreId,
        bookmark: Bookmark,
    },
    // ---- folders
    InsertFolder {
        folder: Folder,
    },
    RemoveFolder {
        folder: FolderId,
    },
    SetFolder {
        folder: Folder,
    },
    // ---- setlists
    InsertList {
        list: Box<Setlist>,
    },
    RemoveList {
        list: ListId,
    },
    RenameList {
        list: ListId,
        name: String,
    },
    InsertEntry {
        list: ListId,
        index: usize,
        entry: ListEntry,
    },
    RemoveEntry {
        list: ListId,
        entry: EntryId,
    },
    SetEntry {
        list: ListId,
        entry: ListEntry,
    },
    MoveEntry {
        list: ListId,
        from: usize,
        to: usize,
    },
    // ---- Recently Deleted and settings
    Trash {
        score: ScoreId,
        at_ms: u64,
    },
    Untrash {
        score: ScoreId,
    },
    SetSetting {
        key: String,
        value: Option<String>,
    },
}

/// The stored record an op changes, so the library store saves only what changed (architecture
/// §8: per record, never a whole-library rewrite).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Touched {
    Score(ScoreId),
    List(ListId),
    /// Folders, Recently Deleted and settings: the library record.
    Library,
}

impl Op {
    /// The record this op changes.
    pub fn touches(&self) -> Touched {
        use Op::*;
        match self {
            InsertScore { score } => Touched::Score(score.id),
            RemoveScore { score }
            | SetMetadata { score, .. }
            | SetScoreFolder { score, .. }
            | SetPageRange { score, .. }
            | SetAudio { score, .. }
            | SetMidiOnOpen { score, .. }
            | SetPracticeNotes { score, .. }
            | InsertPage { score, .. }
            | RemovePage { score, .. }
            | SetPage { score, .. }
            | MovePage { score, .. }
            | InsertLayer { score, .. }
            | RemoveLayer { score, .. }
            | SetLayer { score, .. }
            | MoveLayer { score, .. }
            | InsertAnnotation { score, .. }
            | RemoveAnnotation { score, .. }
            | SetAnnotation { score, .. }
            | InsertBookmark { score, .. }
            | RemoveBookmark { score, .. }
            | SetBookmark { score, .. } => Touched::Score(*score),
            InsertList { list } => Touched::List(list.id),
            RemoveList { list }
            | RenameList { list, .. }
            | InsertEntry { list, .. }
            | RemoveEntry { list, .. }
            | SetEntry { list, .. }
            | MoveEntry { list, .. } => Touched::List(*list),
            InsertFolder { .. }
            | RemoveFolder { .. }
            | SetFolder { .. }
            | Trash { .. }
            | Untrash { .. }
            | SetSetting { .. } => Touched::Library,
        }
    }

    /// Applies the op. On success returns its inverse; on error the library is unchanged.
    pub fn apply(self, lib: &mut Library) -> Result<Op, OpError> {
        use Op::*;
        Ok(match self {
            InsertScore { score } => {
                if lib.scores.contains_key(&score.id) {
                    return Err(OpError::Exists(format!("score {}", score.id)));
                }
                let id = score.id;
                lib.scores.insert(id, *score);
                RemoveScore { score: id }
            }
            RemoveScore { score } => {
                let s = lib.scores.remove(&score).ok_or(OpError::NoScore(score))?;
                InsertScore { score: Box::new(s) }
            }
            SetMetadata { score, metadata } => {
                let s = score_mut(lib, score)?;
                let old = std::mem::replace(&mut s.metadata, *metadata);
                SetMetadata {
                    score,
                    metadata: Box::new(old),
                }
            }
            SetScoreFolder { score, folder } => {
                if let Some(f) = folder
                    && !lib.folders.contains_key(&f)
                {
                    return Err(OpError::NoFolder(f));
                }
                let s = score_mut(lib, score)?;
                let old = std::mem::replace(&mut s.folder, folder);
                SetScoreFolder { score, folder: old }
            }
            SetPageRange { score, range } => {
                let s = score_mut(lib, score)?;
                if let Some(r) = range
                    && r.last as usize >= s.pages.len()
                {
                    return Err(OpError::OutOfRange {
                        what: "page range end",
                        index: r.last as usize,
                        len: s.pages.len(),
                    });
                }
                let old = std::mem::replace(&mut s.page_range, range);
                SetPageRange { score, range: old }
            }
            SetAudio { score, audio } => {
                let s = score_mut(lib, score)?;
                let old = std::mem::replace(&mut s.audio, audio.map(|a| *a));
                SetAudio {
                    score,
                    audio: old.map(Box::new),
                }
            }
            SetMidiOnOpen { score, midi } => {
                let s = score_mut(lib, score)?;
                let old = std::mem::replace(&mut s.midi_on_open, midi);
                SetMidiOnOpen { score, midi: old }
            }
            SetPracticeNotes { score, notes } => {
                let s = score_mut(lib, score)?;
                let old = std::mem::replace(&mut s.practice_notes, notes);
                SetPracticeNotes { score, notes: old }
            }
            InsertPage { score, index, page } => {
                let s = score_mut(lib, score)?;
                check_page_source(s, &page)?;
                check_index(index, s.pages.len() + 1, "page")?;
                s.pages.insert(index, page);
                RemovePage { score, index }
            }
            RemovePage { score, index } => {
                let s = score_mut(lib, score)?;
                check_index(index, s.pages.len(), "page")?;
                // A page range that would fall off the end is the caller's to fix first.
                if let Some(r) = s.page_range
                    && r.last as usize + 1 >= s.pages.len()
                {
                    return Err(OpError::Invalid(
                        "remove the page range before removing pages it covers".into(),
                    ));
                }
                let page = s.pages.remove(index);
                InsertPage { score, index, page }
            }
            SetPage { score, index, page } => {
                let s = score_mut(lib, score)?;
                check_page_source(s, &page)?;
                let len = s.pages.len();
                let slot = s.pages.get_mut(index).ok_or(OpError::OutOfRange {
                    what: "page",
                    index,
                    len,
                })?;
                let old = std::mem::replace(slot, page);
                SetPage {
                    score,
                    index,
                    page: old,
                }
            }
            MovePage { score, from, to } => {
                let s = score_mut(lib, score)?;
                move_item(&mut s.pages, from, to, "page")?;
                MovePage {
                    score,
                    from: to,
                    to: from,
                }
            }
            InsertLayer {
                score,
                index,
                layer,
            } => {
                let s = score_mut(lib, score)?;
                if s.layer(layer.id).is_some() {
                    return Err(OpError::Exists(format!("layer {}", layer.id)));
                }
                check_index(index, s.layers.len() + 1, "layer")?;
                let id = layer.id;
                s.layers.insert(index, layer);
                RemoveLayer { score, layer: id }
            }
            RemoveLayer { score, layer } => {
                let s = score_mut(lib, score)?;
                let index = s
                    .layers
                    .iter()
                    .position(|l| l.id == layer)
                    .ok_or_else(|| OpError::Missing(format!("layer {layer}")))?;
                if s.layers.len() == 1 {
                    return Err(OpError::Invalid("a score keeps at least one layer".into()));
                }
                if s.annotations.values().flatten().any(|a| a.layer == layer) {
                    return Err(OpError::Invalid(
                        "remove the layer's annotations before the layer".into(),
                    ));
                }
                let layer = s.layers.remove(index);
                InsertLayer {
                    score,
                    index,
                    layer,
                }
            }
            SetLayer { score, layer } => {
                let s = score_mut(lib, score)?;
                let slot = s
                    .layers
                    .iter_mut()
                    .find(|l| l.id == layer.id)
                    .ok_or_else(|| OpError::Missing(format!("layer {}", layer.id)))?;
                let old = std::mem::replace(slot, layer);
                SetLayer { score, layer: old }
            }
            MoveLayer { score, from, to } => {
                let s = score_mut(lib, score)?;
                move_item(&mut s.layers, from, to, "layer")?;
                MoveLayer {
                    score,
                    from: to,
                    to: from,
                }
            }
            InsertAnnotation {
                score,
                page,
                index,
                annotation,
            } => {
                let s = score_mut(lib, score)?;
                if page.0 >= s.source.page_count {
                    return Err(OpError::OutOfRange {
                        what: "source page",
                        index: page.0 as usize,
                        len: s.source.page_count as usize,
                    });
                }
                if s.layer(annotation.layer).is_none() {
                    return Err(OpError::Missing(format!("layer {}", annotation.layer)));
                }
                if s.annotations
                    .values()
                    .flatten()
                    .any(|a| a.id == annotation.id)
                {
                    return Err(OpError::Exists(format!("annotation {}", annotation.id)));
                }
                let list = s.annotations.entry(page).or_default();
                if index > list.len() {
                    let len = list.len();
                    if list.is_empty() {
                        s.annotations.remove(&page);
                    }
                    return Err(OpError::OutOfRange {
                        what: "annotation",
                        index,
                        len,
                    });
                }
                let id = annotation.id;
                list.insert(index, *annotation);
                RemoveAnnotation {
                    score,
                    page,
                    annotation: id,
                }
            }
            RemoveAnnotation {
                score,
                page,
                annotation,
            } => {
                let s = score_mut(lib, score)?;
                let missing = || OpError::Missing(format!("annotation {annotation}"));
                let list = s.annotations.get_mut(&page).ok_or_else(missing)?;
                let index = list
                    .iter()
                    .position(|a| a.id == annotation)
                    .ok_or_else(missing)?;
                let a = list.remove(index);
                if list.is_empty() {
                    s.annotations.remove(&page);
                }
                InsertAnnotation {
                    score,
                    page,
                    index,
                    annotation: Box::new(a),
                }
            }
            SetAnnotation {
                score,
                page,
                annotation,
            } => {
                let s = score_mut(lib, score)?;
                if s.layer(annotation.layer).is_none() {
                    return Err(OpError::Missing(format!("layer {}", annotation.layer)));
                }
                let slot = s
                    .annotations
                    .get_mut(&page)
                    .and_then(|l| l.iter_mut().find(|a| a.id == annotation.id))
                    .ok_or_else(|| OpError::Missing(format!("annotation {}", annotation.id)))?;
                let old = std::mem::replace(slot, *annotation);
                SetAnnotation {
                    score,
                    page,
                    annotation: Box::new(old),
                }
            }
            InsertBookmark {
                score,
                index,
                bookmark,
            } => {
                let s = score_mut(lib, score)?;
                if bookmark.page.0 >= s.source.page_count {
                    return Err(OpError::OutOfRange {
                        what: "source page",
                        index: bookmark.page.0 as usize,
                        len: s.source.page_count as usize,
                    });
                }
                if s.bookmarks.iter().any(|b| b.id == bookmark.id) {
                    return Err(OpError::Exists(format!("bookmark {}", bookmark.id)));
                }
                check_index(index, s.bookmarks.len() + 1, "bookmark")?;
                let id = bookmark.id;
                s.bookmarks.insert(index, bookmark);
                RemoveBookmark {
                    score,
                    bookmark: id,
                }
            }
            RemoveBookmark { score, bookmark } => {
                let s = score_mut(lib, score)?;
                let index = s
                    .bookmarks
                    .iter()
                    .position(|b| b.id == bookmark)
                    .ok_or_else(|| OpError::Missing(format!("bookmark {bookmark}")))?;
                let bookmark = s.bookmarks.remove(index);
                InsertBookmark {
                    score,
                    index,
                    bookmark,
                }
            }
            SetBookmark { score, bookmark } => {
                let s = score_mut(lib, score)?;
                if bookmark.page.0 >= s.source.page_count {
                    return Err(OpError::OutOfRange {
                        what: "source page",
                        index: bookmark.page.0 as usize,
                        len: s.source.page_count as usize,
                    });
                }
                let slot = s
                    .bookmarks
                    .iter_mut()
                    .find(|b| b.id == bookmark.id)
                    .ok_or_else(|| OpError::Missing(format!("bookmark {}", bookmark.id)))?;
                let old = std::mem::replace(slot, bookmark);
                SetBookmark {
                    score,
                    bookmark: old,
                }
            }
            InsertFolder { folder } => {
                if lib.folders.contains_key(&folder.id) {
                    return Err(OpError::Exists(format!("folder {}", folder.id)));
                }
                if let Some(p) = folder.parent
                    && !lib.folders.contains_key(&p)
                {
                    return Err(OpError::NoFolder(p));
                }
                let id = folder.id;
                lib.folders.insert(id, folder);
                RemoveFolder { folder: id }
            }
            RemoveFolder { folder } => {
                if !lib.folders.contains_key(&folder) {
                    return Err(OpError::NoFolder(folder));
                }
                let used = lib.scores.values().any(|s| s.folder == Some(folder))
                    || lib.folders.values().any(|f| f.parent == Some(folder));
                if used {
                    return Err(OpError::Invalid(
                        "move the folder's scores and subfolders out first".into(),
                    ));
                }
                let f = lib
                    .folders
                    .remove(&folder)
                    .ok_or(OpError::NoFolder(folder))?;
                InsertFolder { folder: f }
            }
            SetFolder { folder } => {
                if let Some(p) = folder.parent {
                    if !lib.folders.contains_key(&p) {
                        return Err(OpError::NoFolder(p));
                    }
                    if p == folder.id || lib.ancestors(p).contains(&folder.id) {
                        return Err(OpError::Invalid("a folder can't move inside itself".into()));
                    }
                }
                let slot = lib
                    .folders
                    .get_mut(&folder.id)
                    .ok_or(OpError::NoFolder(folder.id))?;
                let old = std::mem::replace(slot, folder);
                SetFolder { folder: old }
            }
            InsertList { list } => {
                if lib.lists.contains_key(&list.id) {
                    return Err(OpError::Exists(format!("list {}", list.id)));
                }
                if let Some(missing) = list
                    .entries
                    .iter()
                    .filter_map(ListEntry::score)
                    .find(|s| !lib.scores.contains_key(s))
                {
                    return Err(OpError::NoScore(missing));
                }
                let id = list.id;
                lib.lists.insert(id, *list);
                RemoveList { list: id }
            }
            RemoveList { list } => {
                let l = lib.lists.remove(&list).ok_or(OpError::NoList(list))?;
                InsertList { list: Box::new(l) }
            }
            RenameList { list, name } => {
                let l = lib.lists.get_mut(&list).ok_or(OpError::NoList(list))?;
                let old = std::mem::replace(&mut l.name, name);
                RenameList { list, name: old }
            }
            InsertEntry { list, index, entry } => {
                if let Some(s) = entry.score()
                    && !lib.scores.contains_key(&s)
                {
                    return Err(OpError::NoScore(s));
                }
                let l = lib.lists.get_mut(&list).ok_or(OpError::NoList(list))?;
                if l.entries.iter().any(|e| e.id == entry.id) {
                    return Err(OpError::Exists(format!("entry {}", entry.id)));
                }
                check_index(index, l.entries.len() + 1, "entry")?;
                let id = entry.id;
                l.entries.insert(index, entry);
                RemoveEntry { list, entry: id }
            }
            RemoveEntry { list, entry } => {
                let l = lib.lists.get_mut(&list).ok_or(OpError::NoList(list))?;
                let index = l
                    .entries
                    .iter()
                    .position(|e| e.id == entry)
                    .ok_or_else(|| OpError::Missing(format!("entry {entry}")))?;
                let entry = l.entries.remove(index);
                InsertEntry { list, index, entry }
            }
            SetEntry { list, entry } => {
                if let Some(s) = entry.score()
                    && !lib.scores.contains_key(&s)
                {
                    return Err(OpError::NoScore(s));
                }
                let l = lib.lists.get_mut(&list).ok_or(OpError::NoList(list))?;
                let slot = l
                    .entries
                    .iter_mut()
                    .find(|e| e.id == entry.id)
                    .ok_or_else(|| OpError::Missing(format!("entry {}", entry.id)))?;
                let old = std::mem::replace(slot, entry);
                SetEntry { list, entry: old }
            }
            MoveEntry { list, from, to } => {
                let l = lib.lists.get_mut(&list).ok_or(OpError::NoList(list))?;
                move_item(&mut l.entries, from, to, "entry")?;
                MoveEntry {
                    list,
                    from: to,
                    to: from,
                }
            }
            Trash { score, at_ms } => {
                if !lib.scores.contains_key(&score) {
                    return Err(OpError::NoScore(score));
                }
                if lib.trash.contains_key(&score) {
                    return Err(OpError::Invalid(format!(
                        "score {score} is already deleted"
                    )));
                }
                lib.trash.insert(score, at_ms);
                Untrash { score }
            }
            Untrash { score } => {
                let at_ms = lib
                    .trash
                    .remove(&score)
                    .ok_or_else(|| OpError::Invalid(format!("score {score} is not deleted")))?;
                Trash { score, at_ms }
            }
            SetSetting { key, value } => {
                let old = match value {
                    Some(v) => lib.settings.insert(key.clone(), v),
                    None => lib.settings.remove(&key),
                };
                SetSetting { key, value: old }
            }
        })
    }
}

fn score_mut(lib: &mut Library, id: ScoreId) -> Result<&mut Score, OpError> {
    lib.scores.get_mut(&id).ok_or(OpError::NoScore(id))
}

fn check_index(index: usize, len: usize, what: &'static str) -> Result<(), OpError> {
    if index < len {
        Ok(())
    } else {
        Err(OpError::OutOfRange {
            what,
            index,
            len: len.saturating_sub(1),
        })
    }
}

fn check_page_source(s: &Score, page: &PageRef) -> Result<(), OpError> {
    if page.source.0 >= s.source.page_count {
        return Err(OpError::OutOfRange {
            what: "source page",
            index: page.source.0 as usize,
            len: s.source.page_count as usize,
        });
    }
    if page.crop.is_some_and(|c| !c.is_finite() || c.is_empty()) {
        return Err(OpError::Invalid(
            "crop must be a non-empty, finite rectangle".into(),
        ));
    }
    Ok(())
}

fn move_item<T>(v: &mut Vec<T>, from: usize, to: usize, what: &'static str) -> Result<(), OpError> {
    check_index(from, v.len(), what)?;
    check_index(to, v.len(), what)?;
    let item = v.remove(from);
    v.insert(to, item);
    Ok(())
}
