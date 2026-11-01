//! Applying one change to a replica: the merge rules of `docs/spec/sync.md`.

use std::collections::BTreeSet;

use scorecraft_doc::{
    Annotation, AnnotationId, Bookmark, BookmarkId, EntryId, Folder, FolderId, Hlc, ListEntry,
    ListId, PageKey, Score, ScoreId, Setlist,
};

use crate::{Change, Elem, Field, Op, Record, Replica};

/// What happened to an element's register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ev {
    /// New content; never revives.
    Edit,
    Remove,
    /// New content, and back if the latest removal is earlier.
    Revive,
}

struct Outcome {
    /// The change's content is now the element's.
    content: bool,
    alive: bool,
}

fn later(a: Option<Hlc>, b: Hlc) -> Option<Hlc> {
    Some(a.map_or(b, |a| a.max(b)))
}

impl Replica {
    /// Whether the score or list a change works inside is here (alive or deleted).
    pub(crate) fn container_known(&self, op: &Op) -> bool {
        match op {
            Op::SetScore { score, .. }
            | Op::PutAnnotation { score, .. }
            | Op::RemoveAnnotation { score, .. }
            | Op::PutBookmark { score, .. }
            | Op::RemoveBookmark { score, .. } => self.score_known(*score),
            Op::PutEntry { list, .. } | Op::RemoveEntry { list, .. } => {
                self.library.lists.contains_key(list) || self.dead.lists.contains_key(list)
            }
            _ => true,
        }
    }

    fn score_known(&self, id: ScoreId) -> bool {
        self.library.scores.contains_key(&id) || self.dead.scores.contains_key(&id)
    }

    /// Apply `change`; false (and nothing changed) when its score or list isn't here yet.
    pub(crate) fn apply(&mut self, change: &Change, changed: &mut BTreeSet<Record>) -> bool {
        if !self.container_known(&change.op) {
            return false;
        }
        let at = change.hlc;
        match &change.op {
            Op::AddScore { score } => {
                let o = self.event(Elem::Score(score.id), Ev::Revive, at);
                let s = self
                    .take_score(score.id)
                    .unwrap_or_else(|| (**score).clone());
                self.place_score(s, o.alive);
                changed.insert(Record::Score(score.id));
            }
            Op::RemoveScore { score } => {
                let o = self.event(Elem::Score(*score), Ev::Remove, at);
                if let Some(s) = self.take_score(*score) {
                    self.place_score(s, o.alive);
                }
                changed.insert(Record::Score(*score));
            }
            Op::SetScore { score, field } => {
                if self.field(Field::Score(*score, field.kind()), at)
                    && let Some(s) = self.score_mut(*score)
                {
                    field.clone().set(s);
                    changed.insert(Record::Score(*score));
                }
            }
            Op::Trash { score, at_ms } => {
                if self.field(Field::Trash(*score), at) {
                    match at_ms {
                        Some(t) => self.library.trash.insert(*score, *t),
                        None => self.library.trash.remove(score),
                    };
                    changed.insert(Record::Library);
                }
            }
            Op::SetSetting { key, value } => {
                if self.field(Field::Setting(key.clone()), at) {
                    match value {
                        Some(v) => self.library.settings.insert(key.clone(), v.clone()),
                        None => self.library.settings.remove(key),
                    };
                    changed.insert(Record::Library);
                }
            }
            Op::PutAnnotation {
                score,
                page,
                annotation,
                revive,
            } => {
                let o = self.event(Elem::Annotation(annotation.id), ev(*revive), at);
                let current = self.take_annotation(*score, annotation.id);
                let content = if o.content {
                    Some((*page, annotation.clone()))
                } else {
                    current
                };
                if let Some((p, a)) = content {
                    self.place_annotation(*score, p, a, o.alive);
                }
                changed.insert(Record::Score(*score));
            }
            Op::RemoveAnnotation { score, annotation } => {
                let o = self.event(Elem::Annotation(*annotation), Ev::Remove, at);
                if let Some((p, a)) = self.take_annotation(*score, *annotation) {
                    self.place_annotation(*score, p, a, o.alive);
                }
                changed.insert(Record::Score(*score));
            }
            Op::PutBookmark {
                score,
                bookmark,
                revive,
            } => {
                let o = self.event(Elem::Bookmark(bookmark.id), ev(*revive), at);
                let current = self.take_bookmark(*score, bookmark.id);
                let content = if o.content {
                    Some(bookmark.clone())
                } else {
                    current
                };
                if let Some(b) = content {
                    self.place_bookmark(*score, b, o.alive);
                }
                changed.insert(Record::Score(*score));
            }
            Op::RemoveBookmark { score, bookmark } => {
                let o = self.event(Elem::Bookmark(*bookmark), Ev::Remove, at);
                if let Some(b) = self.take_bookmark(*score, *bookmark) {
                    self.place_bookmark(*score, b, o.alive);
                }
                changed.insert(Record::Score(*score));
            }
            Op::PutFolder { folder, revive } => {
                let o = self.event(Elem::Folder(folder.id), ev(*revive), at);
                let current = self.take_folder(folder.id);
                let content = if o.content {
                    Some(folder.clone())
                } else {
                    current
                };
                if let Some(f) = content {
                    self.place_folder(f, o.alive);
                }
                changed.insert(Record::Library);
            }
            Op::RemoveFolder { folder } => {
                let o = self.event(Elem::Folder(*folder), Ev::Remove, at);
                if let Some(f) = self.take_folder(*folder) {
                    self.place_folder(f, o.alive);
                }
                changed.insert(Record::Library);
            }
            Op::PutList {
                list,
                name,
                modified_ms,
                revive,
            } => {
                let o = self.event(Elem::List(*list), ev(*revive), at);
                let mut l = self.take_list(*list).unwrap_or_else(|| Setlist {
                    id: *list,
                    name: name.clone(),
                    entries: Vec::new(),
                    modified_ms: *modified_ms,
                });
                if o.content {
                    l.name = name.clone();
                    l.modified_ms = *modified_ms;
                }
                self.place_list(l, o.alive);
                changed.insert(Record::List(*list));
            }
            Op::RemoveList { list } => {
                let o = self.event(Elem::List(*list), Ev::Remove, at);
                if let Some(l) = self.take_list(*list) {
                    self.place_list(l, o.alive);
                }
                changed.insert(Record::List(*list));
            }
            Op::PutEntry {
                list,
                entry,
                pos,
                revive,
            } => {
                let o = self.event(Elem::Entry(entry.id), ev(*revive), at);
                let current = self.take_entry(*list, entry.id);
                let content = if o.content {
                    self.positions.insert(entry.id, pos.clone());
                    Some(entry.clone())
                } else {
                    current
                };
                if let Some(e) = content {
                    self.place_entry(*list, e, o.alive);
                }
                changed.insert(Record::List(*list));
            }
            Op::RemoveEntry { list, entry } => {
                let o = self.event(Elem::Entry(*entry), Ev::Remove, at);
                if let Some(e) = self.take_entry(*list, *entry) {
                    self.place_entry(*list, e, o.alive);
                }
                changed.insert(Record::List(*list));
            }
        }
        true
    }

    fn event(&mut self, e: Elem, ev: Ev, at: Hlc) -> Outcome {
        let r = self.regs.entry(e).or_default();
        let content = ev != Ev::Remove && r.content.is_none_or(|c| at > c);
        if content {
            r.content = Some(at);
        }
        match ev {
            Ev::Edit => {}
            Ev::Remove => r.removed = later(r.removed, at),
            Ev::Revive => r.revived = later(r.revived, at),
        }
        Outcome {
            content,
            alive: r.alive(),
        }
    }

    /// Last-writer-wins: true when `at` is later than the register's clock (which it becomes).
    fn field(&mut self, f: Field, at: Hlc) -> bool {
        match self.fields.get(&f) {
            Some(&won) if won >= at => false,
            _ => {
                self.fields.insert(f, at);
                true
            }
        }
    }

    pub(crate) fn score_mut(&mut self, id: ScoreId) -> Option<&mut Score> {
        match self.library.scores.get_mut(&id) {
            Some(s) => Some(s),
            None => self.dead.scores.get_mut(&id),
        }
    }

    fn list_mut(&mut self, id: ListId) -> Option<&mut Setlist> {
        match self.library.lists.get_mut(&id) {
            Some(l) => Some(l),
            None => self.dead.lists.get_mut(&id),
        }
    }

    fn take_score(&mut self, id: ScoreId) -> Option<Score> {
        self.library
            .scores
            .remove(&id)
            .or_else(|| self.dead.scores.remove(&id))
    }

    fn place_score(&mut self, s: Score, alive: bool) {
        if alive {
            self.library.scores.insert(s.id, s);
        } else {
            self.dead.scores.insert(s.id, s);
        }
    }

    fn take_annotation(
        &mut self,
        score: ScoreId,
        id: AnnotationId,
    ) -> Option<(PageKey, Annotation)> {
        if let Some((_, p, a)) = self.dead.annotations.remove(&id) {
            return Some((p, a));
        }
        let s = self.score_mut(score)?;
        let mut found = None;
        for (page, list) in &mut s.annotations {
            if let Some(i) = list.iter().position(|a| a.id == id) {
                found = Some((*page, list.remove(i)));
                break;
            }
        }
        if let Some((page, _)) = &found
            && s.annotations.get(page).is_some_and(Vec::is_empty)
        {
            s.annotations.remove(page);
        }
        found
    }

    fn place_annotation(&mut self, score: ScoreId, page: PageKey, a: Annotation, alive: bool) {
        if alive && let Some(s) = self.score_mut(score) {
            let list = s.annotations.entry(page).or_default();
            list.push(a);
            list.sort_by_key(|x| x.id);
        } else {
            self.dead.annotations.insert(a.id, (score, page, a));
        }
    }

    fn take_bookmark(&mut self, score: ScoreId, id: BookmarkId) -> Option<Bookmark> {
        if let Some((_, b)) = self.dead.bookmarks.remove(&id) {
            return Some(b);
        }
        let s = self.score_mut(score)?;
        let i = s.bookmarks.iter().position(|b| b.id == id)?;
        Some(s.bookmarks.remove(i))
    }

    fn place_bookmark(&mut self, score: ScoreId, b: Bookmark, alive: bool) {
        if alive && let Some(s) = self.score_mut(score) {
            s.bookmarks.push(b);
            s.bookmarks.sort_by_key(|x| x.id);
        } else {
            self.dead.bookmarks.insert(b.id, (score, b));
        }
    }

    fn take_folder(&mut self, id: FolderId) -> Option<Folder> {
        self.library
            .folders
            .remove(&id)
            .or_else(|| self.dead.folders.remove(&id))
    }

    fn place_folder(&mut self, f: Folder, alive: bool) {
        if alive {
            self.library.folders.insert(f.id, f);
        } else {
            self.dead.folders.insert(f.id, f);
        }
    }

    fn take_list(&mut self, id: ListId) -> Option<Setlist> {
        self.library
            .lists
            .remove(&id)
            .or_else(|| self.dead.lists.remove(&id))
    }

    fn place_list(&mut self, l: Setlist, alive: bool) {
        if alive {
            self.library.lists.insert(l.id, l);
        } else {
            self.dead.lists.insert(l.id, l);
        }
    }

    fn take_entry(&mut self, list: ListId, id: EntryId) -> Option<ListEntry> {
        if let Some((_, e)) = self.dead.entries.remove(&id) {
            return Some(e);
        }
        let l = self.list_mut(list)?;
        let i = l.entries.iter().position(|e| e.id == id)?;
        Some(l.entries.remove(i))
    }

    fn place_entry(&mut self, list: ListId, e: ListEntry, alive: bool) {
        let positions = &self.positions;
        let l = match self.library.lists.get_mut(&list) {
            Some(l) => Some(l),
            None => self.dead.lists.get_mut(&list),
        };
        match l {
            Some(l) if alive => {
                l.entries.push(e);
                l.entries.sort_by(|a, b| {
                    (positions.get(&a.id), a.id).cmp(&(positions.get(&b.id), b.id))
                });
            }
            _ => {
                self.dead.entries.insert(e.id, (list, e));
            }
        }
    }
}

fn ev(revive: bool) -> Ev {
    if revive { Ev::Revive } else { Ev::Edit }
}
