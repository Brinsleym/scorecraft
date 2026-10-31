//! Turning a local edit into changes: compare the edited records with the replica's copy.

use std::collections::{BTreeMap, BTreeSet};

use scorecraft_doc::{Annotation, AnnotationId, EntryId, Library, ListId, PageKey, Score, ScoreId};

use crate::{Elem, Op, Pos, Record, Replica, ScoreField};

impl Replica {
    /// The ops that turn the replica's copy of `records` into `edited`'s.
    pub(crate) fn diff(&self, edited: &Library, records: &[Record]) -> Vec<Op> {
        let mut ops = Vec::new();
        let mut seen = BTreeSet::new();
        for r in records {
            if !seen.insert(*r) {
                continue;
            }
            match r {
                Record::Library => self.diff_library(edited, &mut ops),
                Record::Score(id) => self.diff_score(edited, *id, &mut ops),
                Record::List(id) => self.diff_list(edited, *id, &mut ops),
            }
        }
        // Something removed in one place and put in another (moved between scores or lists)
        // comes back: its put revives it, after the removal.
        let removed: BTreeSet<Elem> = ops.iter().filter_map(removes).collect();
        let (mut moved, mut rest): (Vec<Op>, Vec<Op>) = ops
            .into_iter()
            .partition(|op| puts(op).is_some_and(|e| removed.contains(&e)));
        for op in &mut moved {
            set_revive(op);
        }
        rest.append(&mut moved);
        rest
    }

    fn dead(&self, e: Elem) -> bool {
        self.regs.get(&e).is_some_and(|r| !r.alive())
    }

    fn diff_library(&self, edited: &Library, ops: &mut Vec<Op>) {
        let old = &self.library;
        for (id, f) in &edited.folders {
            if old.folders.get(id) != Some(f) {
                ops.push(Op::PutFolder {
                    folder: f.clone(),
                    revive: self.dead(Elem::Folder(*id)),
                });
            }
        }
        for id in old.folders.keys() {
            if !edited.folders.contains_key(id) {
                ops.push(Op::RemoveFolder { folder: *id });
            }
        }
        let trashed: BTreeSet<ScoreId> = old
            .trash
            .keys()
            .chain(edited.trash.keys())
            .copied()
            .collect();
        for id in trashed {
            let now = edited.trash.get(&id);
            if old.trash.get(&id) != now {
                ops.push(Op::Trash {
                    score: id,
                    at_ms: now.copied(),
                });
            }
        }
        let keys: BTreeSet<&String> = old.settings.keys().chain(edited.settings.keys()).collect();
        for k in keys {
            let now = edited.settings.get(k);
            if old.settings.get(k) != now {
                ops.push(Op::SetSetting {
                    key: k.clone(),
                    value: now.cloned(),
                });
            }
        }
    }

    fn diff_score(&self, edited: &Library, id: ScoreId, ops: &mut Vec<Op>) {
        let new = edited.scores.get(&id);
        let (old, dead) = match self.library.scores.get(&id) {
            Some(s) => (Some(s), false),
            None => (self.dead.scores.get(&id), true),
        };
        match (new, old) {
            (None, None) => {}
            (None, Some(_)) => {
                if !dead {
                    ops.push(Op::RemoveScore { score: id });
                }
            }
            (Some(n), None) => ops.push(Op::AddScore {
                score: Box::new(n.clone()),
            }),
            (Some(n), Some(o)) => {
                if dead {
                    ops.push(Op::AddScore {
                        score: Box::new(n.clone()),
                    });
                }
                for (a, b) in ScoreField::all(n).into_iter().zip(ScoreField::all(o)) {
                    if a != b {
                        ops.push(Op::SetScore {
                            score: id,
                            field: a,
                        });
                    }
                }
                self.diff_marks(n, o, ops);
            }
        }
    }

    fn diff_marks(&self, n: &Score, o: &Score, ops: &mut Vec<Op>) {
        let (new, old) = (flat(n), flat(o));
        for (id, (p, a)) in &new {
            if old.get(id) != Some(&(*p, *a)) {
                ops.push(Op::PutAnnotation {
                    score: n.id,
                    page: *p,
                    annotation: (*a).clone(),
                    revive: self.dead(Elem::Annotation(*id)),
                });
            }
        }
        for id in old.keys() {
            if !new.contains_key(id) {
                ops.push(Op::RemoveAnnotation {
                    score: n.id,
                    annotation: *id,
                });
            }
        }
        for b in &n.bookmarks {
            if o.bookmarks.iter().find(|x| x.id == b.id) != Some(b) {
                ops.push(Op::PutBookmark {
                    score: n.id,
                    bookmark: b.clone(),
                    revive: self.dead(Elem::Bookmark(b.id)),
                });
            }
        }
        for b in &o.bookmarks {
            if !n.bookmarks.iter().any(|x| x.id == b.id) {
                ops.push(Op::RemoveBookmark {
                    score: n.id,
                    bookmark: b.id,
                });
            }
        }
    }

    fn diff_list(&self, edited: &Library, id: ListId, ops: &mut Vec<Op>) {
        let new = edited.lists.get(&id);
        let (old, dead) = match self.library.lists.get(&id) {
            Some(l) => (Some(l), false),
            None => (self.dead.lists.get(&id), true),
        };
        let Some(n) = new else {
            if old.is_some() && !dead {
                ops.push(Op::RemoveList { list: id });
            }
            return;
        };
        if old.is_none_or(|o| dead || o.name != n.name || o.modified_ms != n.modified_ms) {
            ops.push(Op::PutList {
                list: id,
                name: n.name.clone(),
                modified_ms: n.modified_ms,
                revive: old.is_some() && dead,
            });
        }
        let old_entries = old.map_or(&[][..], |o| o.entries.as_slice());
        for e in old_entries {
            if !n.entries.iter().any(|x| x.id == e.id) {
                ops.push(Op::RemoveEntry {
                    list: id,
                    entry: e.id,
                });
            }
        }
        // Entries already here keep their places where they can: the longest run, in the new
        // order, whose positions already increase. The rest get new positions between their
        // neighbours.
        let here: Vec<(usize, (&Pos, EntryId))> = n
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| old_entries.iter().any(|x| x.id == e.id))
            .filter_map(|(i, e)| self.positions.get(&e.id).map(|p| (i, (p, e.id))))
            .collect();
        let kept: BTreeMap<usize, (&Pos, EntryId)> =
            longest_increasing(&here).into_iter().collect();
        let mut prev: Option<Pos> = None;
        for (i, e) in n.entries.iter().enumerate() {
            if let Some((p, _)) = kept.get(&i) {
                if old_entries.iter().find(|x| x.id == e.id) != Some(e) {
                    ops.push(Op::PutEntry {
                        list: id,
                        entry: e.clone(),
                        pos: (*p).clone(),
                        revive: false,
                    });
                }
                prev = Some((*p).clone());
            } else {
                let upper = kept.range(i..).next().map(|(_, (p, _))| *p);
                let p = Pos::between_for(prev.as_ref(), upper, salt(e.id));
                ops.push(Op::PutEntry {
                    list: id,
                    entry: e.clone(),
                    pos: p.clone(),
                    revive: self.dead(Elem::Entry(e.id)),
                });
                prev = Some(p);
            }
        }
    }
}

/// Every marking of `s` by id, with its page.
fn flat(s: &Score) -> BTreeMap<AnnotationId, (PageKey, &Annotation)> {
    let mut m = BTreeMap::new();
    for (p, list) in &s.annotations {
        for a in list {
            m.insert(a.id, (*p, a));
        }
    }
    m
}

/// A digit from an entry's id, so two devices placing entries in the same gap at once give
/// them different positions.
fn salt(id: EntryId) -> u32 {
    (id.as_uuid().as_u128() as u32) | 1
}

/// The longest subsequence of `items` whose keys strictly increase.
fn longest_increasing<T: Ord + Copy>(items: &[(usize, T)]) -> Vec<(usize, T)> {
    // best[i]: (length of the longest run ending at i, the item before i in it).
    let mut best: Vec<(usize, Option<usize>)> = Vec::with_capacity(items.len());
    for (i, (_, p)) in items.iter().enumerate() {
        let mut top = (1, None);
        for (j, (_, q)) in items.iter().enumerate().take(i) {
            let len = best.get(j).map_or(0, |b| b.0);
            if *q < *p && len + 1 > top.0 {
                top = (len + 1, Some(j));
            }
        }
        best.push(top);
    }
    let mut end = best
        .iter()
        .enumerate()
        .max_by_key(|(i, b)| (b.0, std::cmp::Reverse(*i)))
        .map(|(i, _)| i);
    let mut out = Vec::new();
    while let Some(i) = end {
        if let Some(item) = items.get(i) {
            out.push(*item);
        }
        end = best.get(i).and_then(|b| b.1);
    }
    out.reverse();
    out
}

fn removes(op: &Op) -> Option<Elem> {
    match op {
        Op::RemoveAnnotation { annotation, .. } => Some(Elem::Annotation(*annotation)),
        Op::RemoveBookmark { bookmark, .. } => Some(Elem::Bookmark(*bookmark)),
        Op::RemoveEntry { entry, .. } => Some(Elem::Entry(*entry)),
        _ => None,
    }
}

fn puts(op: &Op) -> Option<Elem> {
    match op {
        Op::PutAnnotation { annotation, .. } => Some(Elem::Annotation(annotation.id)),
        Op::PutBookmark { bookmark, .. } => Some(Elem::Bookmark(bookmark.id)),
        Op::PutEntry { entry, .. } => Some(Elem::Entry(entry.id)),
        _ => None,
    }
}

fn set_revive(op: &mut Op) {
    if let Op::PutAnnotation { revive, .. }
    | Op::PutBookmark { revive, .. }
    | Op::PutEntry { revive, .. } = op
    {
        *revive = true;
    }
}
