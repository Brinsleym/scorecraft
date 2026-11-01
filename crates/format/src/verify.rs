//! Verify & Repair Library (LIB-12): check every record and blob, then fix what can be fixed
//! without losing the user's work.

use std::collections::BTreeSet;

use scorecraft_doc::{BlobRef, EntryItem, Layer, LayerId, Library, PageRef};
use sha2::{Digest, Sha256};

use crate::records::paths;
use crate::{Storage, Store, StoreError};

/// Every blob the library refers to.
pub fn referenced_blobs(lib: &Library) -> BTreeSet<BlobRef> {
    let mut out = BTreeSet::new();
    for s in lib.scores.values() {
        out.insert(s.source.blob);
        out.extend(s.source.more.iter().copied());
        if let Some(a) = &s.audio {
            out.insert(a.blob);
        }
    }
    out
}

/// What a verify found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifyReport {
    /// Integrity problems in the model (dangling references, bad numbers), as `path: message`.
    pub problems: Vec<String>,
    /// Blobs a score needs that are not stored.
    pub missing_blobs: Vec<BlobRef>,
    /// Stored blobs whose bytes no longer match their address.
    pub corrupt_blobs: Vec<BlobRef>,
    /// Stored blobs nothing refers to (freed by emptying Recently Deleted).
    pub unused_blobs: Vec<BlobRef>,
    /// Record files that could not be read when the library opened.
    pub unreadable: Vec<String>,
}

impl VerifyReport {
    /// Nothing that needs repair (unused blobs are harmless).
    pub fn is_clean(&self) -> bool {
        self.problems.is_empty()
            && self.missing_blobs.is_empty()
            && self.corrupt_blobs.is_empty()
            && self.unreadable.is_empty()
    }
}

/// What a repair changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepairReport {
    /// One line per fix.
    pub fixes: Vec<String>,
    /// Unreadable records moved to `damaged/`.
    pub quarantined: Vec<String>,
}

impl<S: Storage> Store<S> {
    /// Checks the library against what is stored. `unreadable` is the open report's skipped
    /// records. Reads every blob to check its hash, so it takes a while on a big library.
    pub fn verify(&self, lib: &Library, unreadable: &[String]) -> Result<VerifyReport, StoreError> {
        let mut r = VerifyReport {
            problems: lib
                .problems()
                .into_iter()
                .map(|p| format!("{}: {}", p.path, p.message))
                .collect(),
            unreadable: unreadable.to_vec(),
            ..VerifyReport::default()
        };
        let used = referenced_blobs(lib);
        let stored: BTreeSet<BlobRef> = self.blobs()?.into_iter().collect();
        for b in &used {
            if !stored.contains(b) {
                r.missing_blobs.push(*b);
            }
        }
        for b in &stored {
            if !used.contains(b) {
                r.unused_blobs.push(*b);
            }
            if let Some(bytes) = self.storage().read(&paths::blob(*b))? {
                let d: [u8; 32] = Sha256::digest(&bytes).into();
                if d != b.0 {
                    r.corrupt_blobs.push(*b);
                }
            }
        }
        Ok(r)
    }

    /// Repairs `lib` so it passes every integrity check, moves unreadable records aside, removes
    /// corrupt blobs (their scores then show as missing their file rather than rendering garbage),
    /// and saves. Markings are moved, never dropped, where there is somewhere to move them.
    pub fn repair(
        &mut self,
        lib: &mut Library,
        unreadable: &[String],
    ) -> Result<RepairReport, StoreError> {
        let mut report = RepairReport::default();
        for path in unreadable {
            if crate::storage::check_path(path).is_ok() && path != paths::MANIFEST {
                self.quarantine(path)?;
                report.quarantined.push(path.clone());
            }
        }
        let verify = self.verify(lib, &[])?;
        for b in verify.corrupt_blobs {
            self.storage_mut().remove(&paths::blob(b))?;
            report
                .fixes
                .push(format!("removed damaged file {}", b.to_hex()));
        }
        report.fixes.extend(repair_model(lib));
        self.save_all(lib)?;
        Ok(report)
    }
}

/// Fixes every integrity problem in the model in place, returning one line per fix. After it,
/// `lib.problems()` is empty.
pub fn repair_model(lib: &mut Library) -> Vec<String> {
    let mut fixes = Vec::new();
    // Folders: wrong keys, missing parents, cycles.
    let keys: Vec<_> = lib.folders.keys().copied().collect();
    for k in &keys {
        if let Some(f) = lib.folders.get_mut(k)
            && f.id != *k
        {
            f.id = *k;
            fixes.push(format!("folder {k}: fixed its id"));
        }
    }
    for k in &keys {
        let parent = lib.folders.get(k).and_then(|f| f.parent);
        let bad = match parent {
            Some(p) => !lib.folders.contains_key(&p) || p == *k || lib.ancestors(p).contains(k),
            None => false,
        };
        if bad && let Some(f) = lib.folders.get_mut(k) {
            f.parent = None;
            fixes.push(format!("folder {k}: moved to the top level"));
        }
    }
    let folder_ids: BTreeSet<_> = lib.folders.keys().copied().collect();
    let score_keys: Vec<_> = lib.scores.keys().copied().collect();
    for k in score_keys {
        let Some(s) = lib.scores.get_mut(&k) else {
            continue;
        };
        let at = format!("score {k}");
        if s.id != k {
            s.id = k;
            fixes.push(format!("{at}: fixed its id"));
        }
        if s.folder.is_some_and(|f| !folder_ids.contains(&f)) {
            s.folder = None;
            fixes.push(format!(
                "{at}: moved to the library root (its folder was missing)"
            ));
        }
        let n = s.source.page_count;
        let before = s.pages.len();
        s.pages.retain(|p| p.source.0 < n);
        for p in &mut s.pages {
            if p.crop.is_some_and(|c| !c.is_finite() || c.is_empty()) {
                p.crop = None;
            }
        }
        if s.pages.is_empty() && n > 0 {
            s.pages = (0..n).map(PageRef::new).collect();
        }
        if s.pages.len() != before {
            fixes.push(format!("{at}: page list rebuilt"));
        }
        if s.page_range
            .is_some_and(|r| r.first > r.last || r.last as usize >= s.pages.len())
        {
            s.page_range = None;
            fixes.push(format!("{at}: page range cleared"));
        }
        let mut seen = BTreeSet::new();
        s.layers.retain(|l| seen.insert(l.id));
        if s.layers.is_empty() {
            s.layers.push(Layer {
                id: LayerId(k.0),
                name: "Layer 1".into(),
                visible: true,
                locked: false,
            });
            fixes.push(format!("{at}: added a layer"));
        }
        let first_layer = s.layers.first().map(|l| l.id);
        let layers: BTreeSet<_> = s.layers.iter().map(|l| l.id).collect();
        let mut ann_ids = BTreeSet::new();
        let mut dropped = 0usize;
        let mut moved = 0usize;
        s.annotations.retain(|page, _| {
            page.0 < n || {
                dropped += 1;
                false
            }
        });
        for anns in s.annotations.values_mut() {
            anns.retain(|a| {
                let keep = ann_ids.insert(a.id) && crate::verify::annotation_ok(a, n);
                if !keep {
                    dropped += 1;
                }
                keep
            });
            for a in anns.iter_mut() {
                if !layers.contains(&a.layer)
                    && let Some(l) = first_layer
                {
                    a.layer = l;
                    moved += 1;
                }
            }
        }
        s.annotations.retain(|_, v| !v.is_empty());
        if moved > 0 {
            fixes.push(format!("{at}: {moved} markings moved to the first layer"));
        }
        if dropped > 0 {
            fixes.push(format!("{at}: {dropped} damaged markings removed"));
        }
        let b = s.bookmarks.len();
        s.bookmarks.retain(|bm| bm.page.0 < n);
        if s.bookmarks.len() != b {
            fixes.push(format!("{at}: bookmarks to missing pages removed"));
        }
        if let Some(a) = &mut s.audio
            && !(a.speed.is_finite() && a.speed > 0.0)
        {
            a.speed = 1.0;
            fixes.push(format!("{at}: audio speed reset"));
        }
    }
    let scores: BTreeSet<_> = lib.scores.keys().copied().collect();
    let page_counts: std::collections::BTreeMap<_, _> = lib
        .scores
        .iter()
        .map(|(k, s)| (*k, s.pages.len()))
        .collect();
    let list_keys: Vec<_> = lib.lists.keys().copied().collect();
    for k in list_keys {
        let Some(l) = lib.lists.get_mut(&k) else {
            continue;
        };
        if l.id != k {
            l.id = k;
        }
        let before = l.entries.len();
        let mut seen = BTreeSet::new();
        l.entries
            .retain(|e| seen.insert(e.id) && e.score().is_none_or(|s| scores.contains(&s)));
        for e in &mut l.entries {
            if let EntryItem::Score {
                score,
                page_range,
                tempo,
            } = &mut e.item
            {
                let len = page_counts.get(score).copied().unwrap_or(0);
                if page_range.is_some_and(|r| r.first > r.last || r.last as usize >= len) {
                    *page_range = None;
                }
                if tempo.is_some_and(|t| !(t.is_finite() && t > 0.0 && t <= 1000.0)) {
                    *tempo = None;
                }
            }
        }
        if l.entries.len() != before {
            fixes.push(format!("list {k}: entries for missing scores removed"));
        }
    }
    let t = lib.trash.len();
    lib.trash.retain(|id, _| scores.contains(id));
    if lib.trash.len() != t {
        fixes.push("Recently Deleted: entries for missing scores removed".into());
    }
    fixes
}

pub(crate) fn annotation_ok(a: &scorecraft_doc::Annotation, pages: u32) -> bool {
    use scorecraft_doc::AnnotationKind as K;
    match &a.kind {
        K::Stroke(s) => {
            s.width.is_finite() && s.points.iter().all(scorecraft_doc::InkPoint::is_finite)
        }
        K::Text(t) => t.rect.is_finite() && t.style.size.is_finite(),
        K::Stamp(s) => s.rect.is_finite() && s.size.is_finite(),
        K::Hairpin(h) => {
            h.from.is_finite() && h.to.is_finite() && h.opening.is_finite() && h.width.is_finite()
        }
        K::Snip(s) => {
            s.from.rect.is_finite()
                && s.to.rect.is_finite()
                && s.from.page.0 < pages
                && s.to.page.0 < pages
        }
        K::Shortcut(s) => {
            s.at.rect.is_finite()
                && s.target_rect.is_none_or(|r| r.is_finite())
                && s.at.page.0 < pages
                && s.target.0 < pages
        }
    }
}
