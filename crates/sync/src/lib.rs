//! scorecraft-sync: Sync protocol, CRDT merge, backends (folder, WebDAV, ScoreCraft server) and backup.
//!
//! Layer L4 in the workspace map (`docs/architecture.md` §2–3). Milestone M8 part 1 is the merge
//! core (`docs/spec/sync.md`, SYNC-3 and SYNC-6): a [`Replica`] is one device's library plus what
//! it knows of every device's [`Change`]s. Each device applies its own changes at once and logs
//! them; two replicas exchange "what the other hasn't seen" ([`Replica::summary`],
//! [`Replica::changes_for`]) and, once every change has reached every replica, their libraries
//! are equal, whatever order the changes came in and however often.
//!
//! The rules:
//! - markings are a set of ids with tombstones: a removed marking is gone for good, and
//!   strokes drawn on two devices are both kept;
//! - a marking's content is a last-writer-wins register keyed by the change's hybrid logical
//!   clock (ties break by device, which the clock orders on);
//! - markings on a page are kept in id order (ids are time-ordered), the same on every device;
//! - score details are a last-writer-wins register per score;
//! - a change for a score this replica doesn't have yet waits until the score arrives.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use scorecraft_doc::{
    Annotation, AnnotationId, DeviceId, Hlc, Library, Metadata, PageKey, Score, ScoreId,
};

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

/// What a change does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Op {
    /// A score arrives (imported on another device). A score's id is unique, so adding it twice
    /// is adding it once.
    AddScore { score: Box<Score> },
    /// Add a marking, or replace one (move, recolour, new text) — on `page`, which may differ
    /// from the page it was on.
    PutAnnotation {
        score: ScoreId,
        page: PageKey,
        annotation: Annotation,
    },
    /// Delete a marking for good.
    RemoveAnnotation {
        score: ScoreId,
        annotation: AnnotationId,
    },
    /// Replace a score's details.
    SetMetadata { score: ScoreId, metadata: Metadata },
}

impl Op {
    /// The score the change is about.
    pub fn score(&self) -> ScoreId {
        match self {
            Op::AddScore { score } => score.id,
            Op::PutAnnotation { score, .. }
            | Op::RemoveAnnotation { score, .. }
            | Op::SetMetadata { score, .. } => *score,
        }
    }
}

/// What a replica has seen: for each device, the highest sequence number it has every change
/// up to. Sent to a peer, which answers with the rest ([`Replica::changes_for`]).
pub type Summary = BTreeMap<DeviceId, u64>;

/// One device's library and its knowledge of everyone's changes.
#[derive(Clone, Debug)]
pub struct Replica {
    device: DeviceId,
    library: Library,
    /// Every change this replica has, applied or waiting, by id.
    log: BTreeMap<ChangeId, Change>,
    /// Changes waiting for their score to arrive.
    pending: BTreeSet<ChangeId>,
    tombstones: BTreeSet<AnnotationId>,
    /// The clock of the change that last set each marking's content.
    annotation_clock: BTreeMap<AnnotationId, Hlc>,
    /// The clock of the change that last set each score's details.
    metadata_clock: BTreeMap<ScoreId, Hlc>,
    clock: Hlc,
    seq: u64,
}

impl Replica {
    /// A replica on `device` starting from `library` (every replica of one library starts from
    /// the same copy, or from nothing and receives the scores as changes).
    pub fn new(device: DeviceId, library: Library) -> Replica {
        Replica {
            device,
            library,
            log: BTreeMap::new(),
            pending: BTreeSet::new(),
            tombstones: BTreeSet::new(),
            annotation_clock: BTreeMap::new(),
            metadata_clock: BTreeMap::new(),
            clock: Hlc::new(0, 0, device),
            seq: 0,
        }
    }

    pub fn device(&self) -> DeviceId {
        self.device
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    /// How many received changes wait for a score this replica doesn't have.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Whether `annotation` was deleted (here or anywhere a change came from).
    pub fn is_removed(&self, annotation: AnnotationId) -> bool {
        self.tombstones.contains(&annotation)
    }

    /// Make a change on this device at wall time `now_ms`: it applies at once (or waits for its
    /// score) and is logged for peers. A put marking is stamped with the change's clock.
    pub fn local(&mut self, mut op: Op, now_ms: u64) -> Change {
        self.clock = self.clock.tick(now_ms);
        self.seq = self.seq.saturating_add(1);
        if let Op::PutAnnotation { annotation, .. } = &mut op {
            annotation.stamp = self.clock;
        }
        let change = Change {
            id: ChangeId {
                device: self.device,
                seq: self.seq,
            },
            hlc: self.clock,
            op,
        };
        self.take(change.clone());
        change
    }

    /// Receive changes from a peer at wall time `now_ms`. Changes already seen are ignored;
    /// order doesn't matter. Returns how many were new.
    pub fn receive(&mut self, changes: &[Change], now_ms: u64) -> usize {
        let mut new = 0;
        for c in changes {
            if self.log.contains_key(&c.id) {
                continue;
            }
            self.clock = self.clock.receive(c.hlc, now_ms);
            if c.id.device == self.device {
                // Our own change coming back (e.g. restored from a backup): don't reuse its seq.
                self.seq = self.seq.max(c.id.seq);
            }
            self.take(c.clone());
            new += 1;
        }
        new
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

    fn take(&mut self, change: Change) {
        let id = change.id;
        let applied = self.apply(&change);
        let adds_score = matches!(change.op, Op::AddScore { .. });
        self.log.insert(id, change);
        if !applied {
            self.pending.insert(id);
        } else if adds_score {
            // A score arriving can unblock changes that waited for it.
            self.retry_pending();
        }
    }

    fn retry_pending(&mut self) {
        let waiting: Vec<Change> = self
            .pending
            .iter()
            .filter_map(|id| self.log.get(id).cloned())
            .collect();
        for c in waiting {
            if self.apply(&c) {
                self.pending.remove(&c.id);
            }
        }
    }

    /// Apply one change; false when its score isn't here yet (it then waits).
    fn apply(&mut self, change: &Change) -> bool {
        let score_id = change.op.score();
        if let Op::AddScore { score } = &change.op {
            self.library
                .scores
                .entry(score.id)
                .or_insert_with(|| (**score).clone());
            return true;
        }
        let Some(score) = self.library.scores.get_mut(&score_id) else {
            return false;
        };
        match &change.op {
            Op::AddScore { .. } => {}
            Op::PutAnnotation {
                page, annotation, ..
            } => {
                let id = annotation.id;
                if self.tombstones.contains(&id) {
                    return true;
                }
                if self
                    .annotation_clock
                    .get(&id)
                    .is_some_and(|&won| won >= change.hlc)
                {
                    return true;
                }
                self.annotation_clock.insert(id, change.hlc);
                remove_annotation(score, id);
                let list = score.annotations.entry(*page).or_default();
                list.push(annotation.clone());
                list.sort_by_key(|a| a.id);
            }
            Op::RemoveAnnotation { annotation, .. } => {
                self.tombstones.insert(*annotation);
                self.annotation_clock.remove(annotation);
                remove_annotation(score, *annotation);
            }
            Op::SetMetadata { metadata, .. } => {
                if self
                    .metadata_clock
                    .get(&score_id)
                    .is_some_and(|&won| won >= change.hlc)
                {
                    return true;
                }
                self.metadata_clock.insert(score_id, change.hlc);
                score.metadata = metadata.clone();
            }
        }
        true
    }
}

/// Take marking `id` off whichever page of `score` it is on (dropping the page's entry if it
/// empties, so libraries compare equal however they got there).
fn remove_annotation(score: &mut Score, id: AnnotationId) {
    let mut emptied = None;
    for (page, list) in &mut score.annotations {
        let before = list.len();
        list.retain(|a| a.id != id);
        if list.len() != before {
            if list.is_empty() {
                emptied = Some(*page);
            }
            break;
        }
    }
    if let Some(page) = emptied {
        score.annotations.remove(&page);
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
