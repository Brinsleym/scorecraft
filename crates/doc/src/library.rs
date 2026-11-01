//! The library: folders, scores, setlists, Recently Deleted and library-wide settings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{EntryId, FolderId, LibraryId, ListId, PageRange, Score, ScoreId};

/// A library folder (LIB-3). Folders form a tree through `parent`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: FolderId,
    pub name: String,
    /// `None` for a top-level folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<FolderId>,
    /// When it was deleted (Unix milliseconds): it is in Recently Deleted, read-only, and is
    /// removed for good 30 days later. `None` for a folder in use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_ms: Option<u64>,
}

/// What a setlist entry plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EntryItem {
    /// A score from the library (LIST-6: lists refer to scores, never copy them).
    Score {
        score: ScoreId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page_range: Option<PageRange>,
        /// Beats per minute for the metronome (LIST-3).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tempo: Option<f32>,
    },
    /// A break in the programme: an interval, a spoken item (LIST-7). Newer placeholders carry
    /// a page of their own (`page`, a hidden library score made from the blank A4 template with
    /// the label on it) that playing stops on and that takes ink and text like any score; older
    /// ones have none and are passed over.
    Placeholder {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<ScoreId>,
    },
}

/// One entry of a setlist.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListEntry {
    pub id: EntryId,
    #[serde(flatten)]
    pub item: EntryItem,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl ListEntry {
    pub fn score(&self) -> Option<ScoreId> {
        match self.item {
            EntryItem::Score { score, .. } => Some(score),
            EntryItem::Placeholder { .. } => None,
        }
    }
}

/// A setlist (LIST-1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setlist {
    pub id: ListId,
    pub name: String,
    pub entries: Vec<ListEntry>,
    #[serde(default)]
    pub modified_ms: u64,
}

/// The whole library. Pure data; `scorecraft-format` stores it, `scorecraft-ops` edits it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub id: LibraryId,
    #[serde(default)]
    pub folders: BTreeMap<FolderId, Folder>,
    #[serde(default)]
    pub scores: BTreeMap<ScoreId, Score>,
    #[serde(default)]
    pub lists: BTreeMap<ListId, Setlist>,
    /// Recently Deleted (LIB-10): the score stays in `scores` until the trash is emptied, so a
    /// restore is exact. Value: when it was deleted, Unix milliseconds.
    #[serde(default)]
    pub trash: BTreeMap<ScoreId, u64>,
    /// Library-wide settings that sync with the library (string values; typed by the engine).
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

impl Library {
    pub fn new(id: LibraryId) -> Library {
        Library {
            id,
            folders: BTreeMap::new(),
            scores: BTreeMap::new(),
            lists: BTreeMap::new(),
            trash: BTreeMap::new(),
            settings: BTreeMap::new(),
        }
    }

    /// Scores not in Recently Deleted.
    pub fn live_scores(&self) -> impl Iterator<Item = &Score> {
        self.scores
            .values()
            .filter(|s| !self.trash.contains_key(&s.id))
    }

    pub fn is_trashed(&self, id: ScoreId) -> bool {
        self.trash.contains_key(&id)
    }

    /// The folder or one above it is in Recently Deleted.
    pub fn folder_deleted(&self, folder: FolderId) -> bool {
        std::iter::once(folder)
            .chain(self.ancestors(folder))
            .any(|f| self.folders.get(&f).is_some_and(|f| f.deleted_ms.is_some()))
    }

    /// Folders not in Recently Deleted (nor inside one that is).
    pub fn live_folders(&self) -> impl Iterator<Item = &Folder> {
        self.folders.values().filter(|f| !self.folder_deleted(f.id))
    }

    /// The pages of list placeholders: library scores that belong to a list entry and are kept
    /// out of the library's own listings, search and recent list.
    pub fn placeholder_pages(&self) -> std::collections::BTreeSet<ScoreId> {
        self.lists
            .values()
            .flat_map(|l| &l.entries)
            .filter_map(|e| match e.item {
                EntryItem::Placeholder { page, .. } => page,
                EntryItem::Score { .. } => None,
            })
            .collect()
    }

    /// The folder's ancestors, nearest first, stopping at a cycle (a corrupt file must not loop).
    pub fn ancestors(&self, folder: FolderId) -> Vec<FolderId> {
        let mut out = Vec::new();
        let mut cur = self.folders.get(&folder).and_then(|f| f.parent);
        while let Some(id) = cur {
            if id == folder || out.contains(&id) {
                break;
            }
            out.push(id);
            cur = self.folders.get(&id).and_then(|f| f.parent);
        }
        out
    }

    /// `folder`'s path from the root, `/`-separated (for display and search).
    pub fn folder_path(&self, folder: FolderId) -> String {
        let mut names: Vec<&str> = self
            .ancestors(folder)
            .iter()
            .rev()
            .filter_map(|id| self.folders.get(id).map(|f| f.name.as_str()))
            .collect();
        if let Some(f) = self.folders.get(&folder) {
            names.push(&f.name);
        }
        names.join("/")
    }

    /// Lists that contain the score (so deleting it can say where it's used).
    pub fn lists_using(&self, score: ScoreId) -> Vec<ListId> {
        self.lists
            .values()
            .filter(|l| l.entries.iter().any(|e| e.score() == Some(score)))
            .map(|l| l.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn fid(n: u128) -> FolderId {
        FolderId(Uuid::from_u128(n))
    }

    #[test]
    fn folder_paths_and_cycles() {
        let mut lib = Library::new(LibraryId(Uuid::from_u128(9)));
        for (id, name, parent) in [(1, "Horn", None), (2, "Etudes", Some(fid(1)))] {
            lib.folders.insert(
                fid(id),
                Folder {
                    id: fid(id),
                    name: name.into(),
                    parent,
                    deleted_ms: None,
                },
            );
        }
        assert_eq!(lib.folder_path(fid(2)), "Horn/Etudes");
        // A corrupt file with a cycle.
        if let Some(f) = lib.folders.get_mut(&fid(1)) {
            f.parent = Some(fid(2));
        }
        assert_eq!(lib.ancestors(fid(2)), vec![fid(1)]);
        assert_eq!(lib.folder_path(fid(2)), "Horn/Etudes");
    }

    #[test]
    fn entry_json() {
        let e = ListEntry {
            id: EntryId(Uuid::from_u128(1)),
            item: EntryItem::Placeholder {
                label: "Interval".into(),
                page: None,
            },
            note: String::new(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "placeholder");
        assert_eq!(serde_json::from_value::<ListEntry>(v).unwrap(), e);
    }
}
