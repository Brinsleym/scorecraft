//! scorecraft-sync: Sync protocol, CRDT merge, backends (folder, WebDAV, ScoreCraft server) and backup.
//!
//! Layer L4 in the workspace map (`docs/architecture.md` §2–3). The merge core
//! (`docs/spec/sync.md`, SYNC-3 and SYNC-6): a [`Replica`] is one device's view of a library
//! plus what it knows of every device's [`Change`]s. A device's edits become changes by
//! comparing the edited records with the replica's copy ([`Replica::record`]); they apply at once
//! and are logged. Two replicas exchange "what the other hasn't seen" ([`Replica::summary`],
//! [`Replica::changes_for`]) and, once every change has reached every replica, their libraries
//! are equal, whatever order the changes came in and however often.
//!
//! The rules:
//! - every element (score, marking, bookmark, folder, list, list entry) has a register of three
//!   clocks: its last content change, its last removal and its last revival. Its content is the
//!   latest put; it is gone while its latest removal is later than its latest revival. An edit
//!   never brings back what was deleted (a delete beats a concurrent recolour); an undo of the
//!   delete revives it;
//! - the fields of a score (details, pages, layers, …), Recently Deleted and each setting are
//!   last-writer-wins registers;
//! - markings and bookmarks are kept in id order, list entries in position order ([`Pos`]);
//! - a change inside a score or list this replica doesn't have yet waits until it arrives.
//!
//! Clocks are hybrid logical clocks (`scorecraft_doc::Hlc`), ordered by time, counter and
//! device, so every replica picks the same winner.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod apply;
mod diff;
mod pos;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use scorecraft_doc::{
    Annotation, AnnotationId, AudioTrack, Bookmark, BookmarkId, DeviceId, EntryId, Folder,
    FolderId, Hlc, Layer, Library, ListEntry, ListId, Metadata, MidiOnOpen, PageKey, PageRange,
    PageRef, Score, ScoreId, Setlist,
};

pub use pos::Pos;

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// A change's unique id: the device that made it and its place in that device's log (from 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChangeId {
    pub device: DeviceId,
    pub seq: u64,
}

/// One edit as devices exchange it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub id: ChangeId,
    /// When it was made; orders concurrent edits of the same thing.
    pub hlc: Hlc,
    pub op: Op,
}

/// What a change does. `revive` on a put marks an element coming back after a delete (an undo);
/// a plain put never undoes a delete.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
// Externally tagged: an internally tagged enum buffers its content, and a buffered score loses
// its numeric page keys.
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Op {
    /// A score arrives with everything in it (imported on another device), or comes back.
    AddScore {
        score: Box<Score>,
    },
    RemoveScore {
        score: ScoreId,
    },
    SetScore {
        score: ScoreId,
        field: ScoreField,
    },
    /// Into Recently Deleted (`Some(when)`) or out of it.
    Trash {
        score: ScoreId,
        at_ms: Option<u64>,
    },
    /// Add or replace a marking, on `page` (which may differ from the page it was on).
    PutAnnotation {
        score: ScoreId,
        page: PageKey,
        annotation: Annotation,
        #[serde(default)]
        revive: bool,
    },
    RemoveAnnotation {
        score: ScoreId,
        annotation: AnnotationId,
    },
    PutBookmark {
        score: ScoreId,
        bookmark: Bookmark,
        #[serde(default)]
        revive: bool,
    },
    RemoveBookmark {
        score: ScoreId,
        bookmark: BookmarkId,
    },
    PutFolder {
        folder: Folder,
        #[serde(default)]
        revive: bool,
    },
    RemoveFolder {
        folder: FolderId,
    },
    /// Make a list or rename it.
    PutList {
        list: ListId,
        name: String,
        modified_ms: u64,
        #[serde(default)]
        revive: bool,
    },
    RemoveList {
        list: ListId,
    },
    /// Add, change or move an entry: `pos` places it in the list.
    PutEntry {
        list: ListId,
        entry: ListEntry,
        pos: Pos,
        #[serde(default)]
        revive: bool,
    },
    RemoveEntry {
        list: ListId,
        entry: EntryId,
    },
    SetSetting {
        key: String,
        value: Option<String>,
    },
}

/// A field of a score, set as a whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "field",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScoreField {
    Metadata {
        metadata: Metadata,
    },
    Folder {
        folder: Option<FolderId>,
    },
    /// The page list (order, rotation, crop, hidden) and the page range.
    Pages {
        pages: Vec<PageRef>,
        page_range: Option<PageRange>,
    },
    Layers {
        layers: Vec<Layer>,
    },
    Audio {
        audio: Option<AudioTrack>,
    },
    MidiOnOpen {
        midi: Option<MidiOnOpen>,
    },
    PracticeNotes {
        notes: String,
    },
    Times {
        added_ms: u64,
        opened_ms: Option<u64>,
        modified_ms: u64,
    },
}

impl ScoreField {
    fn kind(&self) -> u8 {
        match self {
            ScoreField::Metadata { .. } => 0,
            ScoreField::Folder { .. } => 1,
            ScoreField::Pages { .. } => 2,
            ScoreField::Layers { .. } => 3,
            ScoreField::Audio { .. } => 4,
            ScoreField::MidiOnOpen { .. } => 5,
            ScoreField::PracticeNotes { .. } => 6,
            ScoreField::Times { .. } => 7,
        }
    }

    /// Every field of `s`.
    pub fn all(s: &Score) -> [ScoreField; 8] {
        [
            ScoreField::Metadata {
                metadata: s.metadata.clone(),
            },
            ScoreField::Folder { folder: s.folder },
            ScoreField::Pages {
                pages: s.pages.clone(),
                page_range: s.page_range,
            },
            ScoreField::Layers {
                layers: s.layers.clone(),
            },
            ScoreField::Audio {
                audio: s.audio.clone(),
            },
            ScoreField::MidiOnOpen {
                midi: s.midi_on_open,
            },
            ScoreField::PracticeNotes {
                notes: s.practice_notes.clone(),
            },
            ScoreField::Times {
                added_ms: s.added_ms,
                opened_ms: s.opened_ms,
                modified_ms: s.modified_ms,
            },
        ]
    }

    fn set(self, s: &mut Score) {
        match self {
            ScoreField::Metadata { metadata } => s.metadata = metadata,
            ScoreField::Folder { folder } => s.folder = folder,
            ScoreField::Pages { pages, page_range } => {
                s.pages = pages;
                s.page_range = page_range;
            }
            ScoreField::Layers { layers } => s.layers = layers,
            ScoreField::Audio { audio } => s.audio = audio,
            ScoreField::MidiOnOpen { midi } => s.midi_on_open = midi,
            ScoreField::PracticeNotes { notes } => s.practice_notes = notes,
            ScoreField::Times {
                added_ms,
                opened_ms,
                modified_ms,
            } => {
                s.added_ms = added_ms;
                s.opened_ms = opened_ms;
                s.modified_ms = modified_ms;
            }
        }
    }
}

/// A stored record of the library (as the library store saves them): what a local edit touched,
/// or what received changes changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Record {
    Score(ScoreId),
    List(ListId),
    /// Folders, Recently Deleted and settings.
    Library,
}

/// What a replica has seen: for each device, the highest sequence number it has every change
/// up to. Sent to a peer, which answers with the rest ([`Replica::changes_for`]).
pub type Summary = BTreeMap<DeviceId, u64>;

/// What receiving changes did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Received {
    /// Changes not seen before.
    pub new: usize,
    /// The records they changed, for the caller to copy into its library and save.
    pub changed: BTreeSet<Record>,
}

/// The things that have an existence register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Elem {
    Score(ScoreId),
    Annotation(AnnotationId),
    Bookmark(BookmarkId),
    Folder(FolderId),
    List(ListId),
    Entry(EntryId),
}

/// The last-writer-wins registers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Field {
    Score(ScoreId, u8),
    Trash(ScoreId),
    Setting(String),
}

/// An element's three clocks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Reg {
    content: Option<Hlc>,
    removed: Option<Hlc>,
    revived: Option<Hlc>,
}

impl Reg {
    pub(crate) fn alive(&self) -> bool {
        match self.removed {
            None => true,
            Some(r) => self.revived.is_some_and(|v| v > r),
        }
    }
}

/// Elements that are deleted (or wait in a deleted container), kept so later changes still
/// apply to them and a revival brings them back as they were.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Dead {
    scores: BTreeMap<ScoreId, Score>,
    annotations: BTreeMap<AnnotationId, (ScoreId, PageKey, Annotation)>,
    bookmarks: BTreeMap<BookmarkId, (ScoreId, Bookmark)>,
    folders: BTreeMap<FolderId, Folder>,
    lists: BTreeMap<ListId, Setlist>,
    entries: BTreeMap<EntryId, (ListId, ListEntry)>,
}

/// Everything a replica keeps besides its library, to save and load with it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReplicaState {
    log: Vec<Change>,
    regs: Vec<(Elem, Reg)>,
    fields: Vec<(Field, Hlc)>,
    positions: Vec<(EntryId, Pos)>,
    dead: Dead,
    clock: Hlc,
    seq: u64,
}

/// One device's library and its knowledge of everyone's changes.
#[derive(Clone, Debug)]
pub struct Replica {
    device: DeviceId,
    library: Library,
    /// Every change this replica has, applied or waiting, by id.
    log: BTreeMap<ChangeId, Change>,
    /// Changes waiting for their score or list to arrive.
    pending: BTreeSet<ChangeId>,
    regs: BTreeMap<Elem, Reg>,
    fields: BTreeMap<Field, Hlc>,
    positions: BTreeMap<EntryId, Pos>,
    dead: Dead,
    clock: Hlc,
    seq: u64,
}

impl Replica {
    /// A replica on `device` starting from `library`. Every replica of one library starts from
    /// the same copy, or (the usual case) from an empty library with the same id, recording its
    /// device's existing library as changes with [`Replica::record_all`].
    pub fn new(device: DeviceId, library: Library) -> Replica {
        let mut positions = BTreeMap::new();
        for list in library.lists.values() {
            for (i, e) in list.entries.iter().enumerate() {
                positions.insert(e.id, Pos::nth(i));
            }
        }
        Replica {
            device,
            library,
            log: BTreeMap::new(),
            pending: BTreeSet::new(),
            regs: BTreeMap::new(),
            fields: BTreeMap::new(),
            positions,
            dead: Dead::default(),
            clock: Hlc::new(0, 0, device),
            seq: 0,
        }
    }

    /// A replica loaded from its saved `library` and `state`.
    pub fn load(device: DeviceId, library: Library, state: ReplicaState) -> Replica {
        let mut r = Replica::new(device, library);
        r.regs = state.regs.into_iter().collect();
        r.fields = state.fields.into_iter().collect();
        r.positions.extend(state.positions);
        r.dead = state.dead;
        r.clock = Hlc::new(state.clock.ms, state.clock.counter, device);
        r.seq = state.seq;
        for c in state.log {
            r.log.insert(c.id, c);
        }
        // Whatever waited still waits until it can apply.
        let waiting: Vec<ChangeId> = r
            .log
            .values()
            .filter(|c| !r.container_known(&c.op))
            .map(|c| c.id)
            .collect();
        r.pending = waiting.into_iter().collect();
        r
    }

    /// What to save besides the library.
    pub fn state(&self) -> ReplicaState {
        ReplicaState {
            log: self.log.values().cloned().collect(),
            regs: self.regs.iter().map(|(k, v)| (*k, *v)).collect(),
            fields: self.fields.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            positions: self
                .positions
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect(),
            dead: self.dead.clone(),
            clock: self.clock,
            seq: self.seq,
        }
    }

    pub fn device(&self) -> DeviceId {
        self.device
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    /// How many received changes wait for a score or list this replica doesn't have.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Whether this replica has change `id` (applied or waiting).
    pub fn knows(&self, id: &ChangeId) -> bool {
        self.log.contains_key(id)
    }

    /// How many changes this replica has, from every device.
    pub fn log_len(&self) -> usize {
        self.log.len()
    }

    /// Whether marking `annotation` is deleted (here or wherever a change came from).
    pub fn is_removed(&self, annotation: AnnotationId) -> bool {
        self.regs
            .get(&Elem::Annotation(annotation))
            .is_some_and(|r| !r.alive())
    }

    /// Make a change on this device at wall time `now_ms`: it applies at once (or waits for its
    /// score) and is logged for peers.
    pub fn local(&mut self, op: Op, now_ms: u64) -> Change {
        self.clock = self.clock.tick(now_ms);
        self.seq = self.seq.saturating_add(1);
        let change = Change {
            id: ChangeId {
                device: self.device,
                seq: self.seq,
            },
            hlc: self.clock,
            op,
        };
        let mut changed = BTreeSet::new();
        self.take(change.clone(), &mut changed);
        change
    }

    /// Record a local edit: compare `records` of `edited` (this device's library after the edit)
    /// with the replica's copy and log a change for each difference.
    pub fn record(&mut self, edited: &Library, records: &[Record], now_ms: u64) -> Vec<Change> {
        let ops = self.diff(edited, records);
        ops.into_iter().map(|op| self.local(op, now_ms)).collect()
    }

    /// Record every difference between `edited` and the replica's copy (a device joining sync
    /// records its whole library this way).
    pub fn record_all(&mut self, edited: &Library, now_ms: u64) -> Vec<Change> {
        let mut records: BTreeSet<Record> = BTreeSet::from([Record::Library]);
        for lib in [edited, &self.library] {
            records.extend(lib.scores.keys().map(|id| Record::Score(*id)));
            records.extend(lib.lists.keys().map(|id| Record::List(*id)));
        }
        let records: Vec<Record> = records.into_iter().collect();
        self.record(edited, &records, now_ms)
    }

    /// Receive changes from a peer at wall time `now_ms`. Changes already seen are ignored;
    /// order doesn't matter.
    pub fn receive(&mut self, changes: &[Change], now_ms: u64) -> Received {
        let mut out = Received::default();
        for c in changes {
            if self.log.contains_key(&c.id) {
                continue;
            }
            self.clock = self.clock.receive(c.hlc, now_ms);
            if c.id.device == self.device {
                // Our own change coming back (e.g. restored from a backup): don't reuse its seq.
                self.seq = self.seq.max(c.id.seq);
            }
            self.take(c.clone(), &mut out.changed);
            out.new += 1;
        }
        self.retry_pending(&mut out.changed);
        out
    }

    /// What this replica has seen, to send to a peer.
    pub fn summary(&self) -> Summary {
        let mut out = Summary::new();
        for id in self.log.keys() {
            let have = out.entry(id.device).or_insert(0);
            if id.seq == have.saturating_add(1) {
                *have = id.seq;
            }
        }
        out
    }

    /// The changes a peer with `summary` hasn't seen, oldest first per device.
    pub fn changes_for(&self, summary: &Summary) -> Vec<Change> {
        self.log
            .values()
            .filter(|c| summary.get(&c.id.device).is_none_or(|&s| c.id.seq > s))
            .cloned()
            .collect()
    }

    /// Copy `records` from the replica's library into `lib` (after [`Replica::receive`], so
    /// this device's library shows what arrived).
    pub fn copy_into(&self, lib: &mut Library, records: &BTreeSet<Record>) {
        for r in records {
            match r {
                Record::Score(id) => match self.library.scores.get(id) {
                    Some(s) => {
                        lib.scores.insert(*id, s.clone());
                    }
                    None => {
                        lib.scores.remove(id);
                    }
                },
                Record::List(id) => match self.library.lists.get(id) {
                    Some(l) => {
                        lib.lists.insert(*id, l.clone());
                    }
                    None => {
                        lib.lists.remove(id);
                    }
                },
                Record::Library => {
                    lib.folders = self.library.folders.clone();
                    lib.trash = self.library.trash.clone();
                    lib.settings = self.library.settings.clone();
                }
            }
        }
    }

    fn take(&mut self, change: Change, changed: &mut BTreeSet<Record>) {
        let id = change.id;
        let applied = self.apply(&change, changed);
        self.log.insert(id, change);
        if !applied {
            self.pending.insert(id);
        } else if !self.pending.is_empty() {
            // A score or list arriving can unblock changes that waited for it.
            self.retry_pending(changed);
        }
    }

    fn retry_pending(&mut self, changed: &mut BTreeSet<Record>) {
        loop {
            let waiting: Vec<Change> = self
                .pending
                .iter()
                .filter_map(|id| self.log.get(id).cloned())
                .collect();
            let mut progress = false;
            for c in waiting {
                if self.apply(&c, changed) {
                    self.pending.remove(&c.id);
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
    }
}

/// Exchange everything between two replicas, both ways, at wall time `now_ms`.
pub fn exchange(a: &mut Replica, b: &mut Replica, now_ms: u64) {
    let to_b = a.changes_for(&b.summary());
    let to_a = b.changes_for(&a.summary());
    b.receive(&to_b, now_ms);
    a.receive(&to_a, now_ms);
}

#[cfg(test)]
mod tests;
