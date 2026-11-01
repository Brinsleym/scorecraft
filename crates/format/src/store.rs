//! The library store: opens, saves and checks a library folder.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use scorecraft_doc::{BlobRef, Library, LibraryId, ListId, ScoreId, Setlist};
use scorecraft_ops::Touched;

use crate::records::{
    FORMAT_NAME, FORMAT_VERSION, InkRecord, LibraryRecord, Manifest, ScoreRecord, join_library,
    join_score, paths, split_library, split_score,
};
use crate::{Storage, StoreError};

/// The largest record file the loader reads (a score's JSON); bigger is treated as damaged
/// rather than parsed. Blobs have no limit.
pub const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;

/// What opening a library found and fixed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Records that could not be read, with why. The rest of the library still opens.
    pub skipped: Vec<(String, String)>,
    /// An interrupted save was finished from the journal.
    pub journal_replayed: bool,
    /// Temp files left by an interrupted write were removed.
    pub temp_files_removed: usize,
}

/// What one save wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveStats {
    pub written: usize,
    pub removed: usize,
    /// The save went through the journal (more than one file changed).
    pub journaled: bool,
}

#[derive(Serialize, Deserialize)]
struct Journal {
    entries: Vec<JournalEntry>,
}

#[derive(Serialize, Deserialize)]
struct JournalEntry {
    path: String,
    /// The record's new JSON, or `None` to remove it.
    #[serde(default)]
    data: Option<String>,
}

/// An open library folder. Saves are per record and atomic; a save that changes more than one
/// record goes through a journal so a crash part-way is finished on the next open, never left
/// half-applied (architecture §8).
pub struct Store<S: Storage> {
    storage: S,
    manifest: Manifest,
    /// SHA-256 of every record as last read or written, so unchanged records aren't rewritten.
    written: BTreeMap<String, [u8; 32]>,
}

fn digest(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn to_json<T: Serialize>(v: &T, what: &str) -> Result<Vec<u8>, StoreError> {
    serde_json::to_vec_pretty(v)
        .map_err(|e| StoreError::Corrupt(format!("could not encode {what}: {e}")))
}

fn parse<T: for<'de> Deserialize<'de>>(path: &str, data: &[u8]) -> Result<T, String> {
    if data.len() > MAX_RECORD_BYTES {
        return Err(format!("{path} is over {MAX_RECORD_BYTES} bytes"));
    }
    serde_json::from_slice(data).map_err(|e| format!("{path}: {e}"))
}

/// Record paths a journal may touch: never the manifest or blobs.
fn journal_may_write(path: &str) -> bool {
    path == paths::LIBRARY
        || [paths::SCORES, paths::LISTS, paths::INK]
            .iter()
            .any(|p| path.starts_with(&format!("{p}/")))
}

impl<S: Storage> Store<S> {
    /// Makes a new, empty library in `storage`. Fails if one is already there.
    pub fn create(
        storage: S,
        id: LibraryId,
        now_ms: u64,
    ) -> Result<(Store<S>, Library), StoreError> {
        if storage.read(paths::MANIFEST)?.is_some() {
            return Err(StoreError::Exists(storage.describe()));
        }
        let manifest = Manifest {
            format: FORMAT_NAME.to_owned(),
            version: FORMAT_VERSION,
            library: id,
            created_ms: now_ms,
        };
        let lib = Library::new(id);
        let mut store = Store {
            storage,
            manifest: manifest.clone(),
            written: BTreeMap::new(),
        };
        store.save_all(&lib)?;
        // The manifest goes last: a crash before it leaves a folder that isn't a library yet,
        // and creating again just works.
        let bytes = to_json(&manifest, "the manifest")?;
        store.storage.write_atomic(paths::MANIFEST, &bytes)?;
        Ok((store, lib))
    }

    /// Opens the library in `storage`: finishes an interrupted save, clears temp files, and loads
    /// every record it can. Unreadable records are skipped and listed in the report; the library
    /// still opens (never crash).
    pub fn open(mut storage: S) -> Result<(Store<S>, Library, LoadReport), StoreError> {
        let manifest_bytes = storage
            .read(paths::MANIFEST)?
            .ok_or_else(|| StoreError::NotALibrary(storage.describe()))?;
        let manifest: Manifest =
            parse(paths::MANIFEST, &manifest_bytes).map_err(StoreError::Corrupt)?;
        if manifest.format != FORMAT_NAME {
            return Err(StoreError::NotALibrary(storage.describe()));
        }
        if manifest.version > FORMAT_VERSION {
            return Err(StoreError::TooNew {
                version: manifest.version,
            });
        }
        let mut report = LoadReport::default();
        report.journal_replayed = replay_journal(&mut storage, &mut report)?;
        for dir in [
            paths::SCORES,
            paths::LISTS,
            paths::INK,
            paths::BLOBS,
            "journal",
        ] {
            for f in storage.list(dir)? {
                if f.ends_with(".tmp") {
                    storage.remove(&f)?;
                    report.temp_files_removed += 1;
                }
            }
        }
        if storage.read(&format!("{}.tmp", paths::LIBRARY))?.is_some() {
            storage.remove(&format!("{}.tmp", paths::LIBRARY))?;
            report.temp_files_removed += 1;
        }

        let mut store = Store {
            storage,
            manifest,
            written: BTreeMap::new(),
        };
        let record: LibraryRecord = store.load(paths::LIBRARY, &mut report).unwrap_or_default();
        let mut scores = Vec::new();
        for path in store.storage.list(paths::SCORES)? {
            if !path.ends_with(".json") {
                continue;
            }
            let Some(rec) = store.load::<ScoreRecord>(&path, &mut report) else {
                continue;
            };
            if paths::score(rec.id) != path {
                report
                    .skipped
                    .push((path.clone(), "file name does not match the score id".into()));
                store.written.remove(&path);
                continue;
            }
            let mut ink = Vec::new();
            for ipath in store.storage.list(&paths::ink_dir(rec.id))? {
                if let Some(r) = store.load::<InkRecord>(&ipath, &mut report) {
                    if paths::ink(rec.id, r.page) == ipath {
                        ink.push(r);
                    } else {
                        report
                            .skipped
                            .push((ipath.clone(), "file name does not match its page".into()));
                        store.written.remove(&ipath);
                    }
                }
            }
            scores.push(join_score(rec, ink));
        }
        let mut lists = Vec::new();
        for path in store.storage.list(paths::LISTS)? {
            if !path.ends_with(".json") {
                continue;
            }
            if let Some(l) = store.load::<Setlist>(&path, &mut report) {
                if paths::list(l.id) == path {
                    lists.push(l);
                } else {
                    report
                        .skipped
                        .push((path.clone(), "file name does not match the list id".into()));
                    store.written.remove(&path);
                }
            }
        }
        let lib = join_library(store.manifest.library, record, scores, lists);
        Ok((store, lib, report))
    }

    fn load<T: for<'de> Deserialize<'de>>(
        &mut self,
        path: &str,
        report: &mut LoadReport,
    ) -> Option<T> {
        let bytes = match self.storage.read(path) {
            Ok(Some(b)) => b,
            Ok(None) => return None,
            Err(e) => {
                report.skipped.push((path.to_owned(), e.to_string()));
                return None;
            }
        };
        match parse(path, &bytes) {
            Ok(v) => {
                self.written.insert(path.to_owned(), digest(&bytes));
                Some(v)
            }
            Err(e) => {
                report.skipped.push((path.to_owned(), e));
                None
            }
        }
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn storage(&self) -> &S {
        &self.storage
    }

    /// Saves the records `touched` names, writing only the files whose contents changed.
    pub fn save(
        &mut self,
        lib: &Library,
        touched: &BTreeSet<Touched>,
    ) -> Result<SaveStats, StoreError> {
        let mut changes: BTreeMap<String, Option<Vec<u8>>> = BTreeMap::new();
        for t in touched {
            match *t {
                Touched::Library => {
                    let (rec, _, _) = split_library(lib);
                    changes.insert(
                        paths::LIBRARY.to_owned(),
                        Some(to_json(&rec, "the library")?),
                    );
                }
                Touched::Score(id) => self.score_changes(lib, id, &mut changes)?,
                Touched::List(id) => {
                    let path = paths::list(id);
                    let data = match lib.lists.get(&id) {
                        Some(l) => Some(to_json(l, "a list")?),
                        None => None,
                    };
                    changes.insert(path, data);
                }
            }
        }
        self.commit(changes)
    }

    /// Saves every record, and removes record files on disk that the library no longer has.
    pub fn save_all(&mut self, lib: &Library) -> Result<SaveStats, StoreError> {
        let mut touched: BTreeSet<Touched> = BTreeSet::from([Touched::Library]);
        touched.extend(lib.scores.keys().map(|id| Touched::Score(*id)));
        touched.extend(lib.lists.keys().map(|id| Touched::List(*id)));
        for path in self.storage.list(paths::SCORES)? {
            if let Some(id) = id_of::<ScoreId>(&path, paths::SCORES) {
                touched.insert(Touched::Score(id));
            }
        }
        for path in self.storage.list(paths::LISTS)? {
            if let Some(id) = id_of::<ListId>(&path, paths::LISTS) {
                touched.insert(Touched::List(id));
            }
        }
        self.save(lib, &touched)
    }

    fn score_changes(
        &mut self,
        lib: &Library,
        id: ScoreId,
        changes: &mut BTreeMap<String, Option<Vec<u8>>>,
    ) -> Result<(), StoreError> {
        let dir = format!("{}/", paths::ink_dir(id));
        // Ink files that exist now: known from loads and saves, plus any on disk.
        let mut old_ink: BTreeSet<String> = self
            .written
            .keys()
            .filter(|p| p.starts_with(&dir))
            .cloned()
            .collect();
        old_ink.extend(self.storage.list(&paths::ink_dir(id))?);
        match lib.scores.get(&id) {
            Some(score) => {
                let parts = split_score(score);
                changes.insert(paths::score(id), Some(to_json(&parts.record, "a score")?));
                for ink in &parts.ink {
                    let p = paths::ink(id, ink.page);
                    old_ink.remove(&p);
                    changes.insert(p, Some(to_json(ink, "a page's annotations")?));
                }
            }
            None => {
                changes.insert(paths::score(id), None);
            }
        }
        for p in old_ink {
            changes.insert(p, None);
        }
        Ok(())
    }

    /// Applies `changes`, skipping ones that change nothing. More than one change goes through
    /// the journal.
    fn commit(
        &mut self,
        changes: BTreeMap<String, Option<Vec<u8>>>,
    ) -> Result<SaveStats, StoreError> {
        let changes: Vec<(String, Option<Vec<u8>>)> = changes
            .into_iter()
            .filter(|(path, data)| match data {
                Some(d) => self.written.get(path) != Some(&digest(d)),
                None => true,
            })
            .collect();
        // A removal of a file that was never there is not a change.
        let mut real = Vec::with_capacity(changes.len());
        for (path, data) in changes {
            if data.is_none()
                && !self.written.contains_key(&path)
                && self.storage.read(&path)?.is_none()
            {
                continue;
            }
            real.push((path, data));
        }
        let mut stats = SaveStats::default();
        if real.is_empty() {
            return Ok(stats);
        }
        if real.len() > 1 {
            let journal = Journal {
                entries: real
                    .iter()
                    .map(|(path, data)| JournalEntry {
                        path: path.clone(),
                        data: data
                            .as_ref()
                            .map(|d| String::from_utf8_lossy(d).into_owned()),
                    })
                    .collect(),
            };
            let bytes = serde_json::to_vec(&journal)
                .map_err(|e| StoreError::Corrupt(format!("could not encode the journal: {e}")))?;
            self.storage.write_atomic(paths::JOURNAL, &bytes)?;
            stats.journaled = true;
        }
        for (path, data) in real {
            match data {
                Some(d) => {
                    self.storage.write_atomic(&path, &d)?;
                    self.written.insert(path, digest(&d));
                    stats.written += 1;
                }
                None => {
                    self.storage.remove(&path)?;
                    self.written.remove(&path);
                    stats.removed += 1;
                }
            }
        }
        if stats.journaled {
            self.storage.remove(paths::JOURNAL)?;
        }
        Ok(stats)
    }

    /// Stores a file's bytes by content and returns its address. Storing the same bytes twice
    /// keeps one copy.
    pub fn put_blob(&mut self, data: &[u8]) -> Result<BlobRef, StoreError> {
        let b = BlobRef(digest(data));
        let path = paths::blob(b);
        if self.storage.read(&path)?.is_none() {
            self.storage.write_atomic(&path, data)?;
        }
        Ok(b)
    }

    /// Derived data kept beside the library under `cache/` (the full-text index): never part of
    /// the model, never read by open or Verify & Repair, and safe to lose.
    pub fn cache(&self, name: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let path = format!("cache/{name}");
        crate::storage::check_path(&path)?;
        self.storage.read(&path)
    }

    /// Stores derived data under `cache/` (see [`Store::cache`]).
    pub fn set_cache(&mut self, name: &str, data: &[u8]) -> Result<(), StoreError> {
        let path = format!("cache/{name}");
        crate::storage::check_path(&path)?;
        self.storage.write_atomic(&path, data)
    }

    /// A stored file's bytes.
    pub fn blob(&self, b: BlobRef) -> Result<Vec<u8>, StoreError> {
        self.storage
            .read(&paths::blob(b))?
            .ok_or_else(|| StoreError::MissingBlob(b.to_hex()))
    }

    pub fn has_blob(&self, b: BlobRef) -> Result<bool, StoreError> {
        Ok(self.storage.read(&paths::blob(b))?.is_some())
    }

    /// Every stored blob.
    pub fn blobs(&self) -> Result<Vec<BlobRef>, StoreError> {
        Ok(self
            .storage
            .list(paths::BLOBS)?
            .iter()
            .filter_map(|p| paths::blob_of(p))
            .collect())
    }

    /// Removes blobs no score refers to (after Recently Deleted is emptied). Returns how many.
    pub fn collect_garbage(&mut self, lib: &Library) -> Result<usize, StoreError> {
        let used = crate::verify::referenced_blobs(lib);
        let mut removed = 0;
        for b in self.blobs()? {
            if !used.contains(&b) {
                self.storage.remove(&paths::blob(b))?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Moves an unreadable record aside to `damaged/` so a repair can save cleanly.
    pub(crate) fn quarantine(&mut self, path: &str) -> Result<(), StoreError> {
        if let Some(bytes) = self.storage.read(path)? {
            let name = path.replace('/', "_");
            self.storage
                .write_atomic(&format!("{}/{name}", paths::DAMAGED), &bytes)?;
            self.storage.remove(path)?;
        }
        self.written.remove(path);
        Ok(())
    }

    pub(crate) fn storage_mut(&mut self) -> &mut S {
        &mut self.storage
    }
}

/// The id in a `<dir>/<id>.json` path.
fn id_of<T: std::str::FromStr>(path: &str, dir: &str) -> Option<T> {
    path.strip_prefix(dir)?
        .strip_prefix('/')?
        .strip_suffix(".json")?
        .parse()
        .ok()
}

/// Finishes a save that a crash interrupted. The journal was written atomically before any
/// record, so it is either whole or absent; a damaged one (planted, or a bad disk) is discarded
/// and the records stay as they were.
fn replay_journal<S: Storage>(
    storage: &mut S,
    report: &mut LoadReport,
) -> Result<bool, StoreError> {
    let Some(bytes) = storage.read(paths::JOURNAL)? else {
        return Ok(false);
    };
    let journal: Journal = match parse(paths::JOURNAL, &bytes) {
        Ok(j) => j,
        Err(e) => {
            report.skipped.push((paths::JOURNAL.to_owned(), e));
            storage.remove(paths::JOURNAL)?;
            return Ok(false);
        }
    };
    for e in &journal.entries {
        if crate::storage::check_path(&e.path).is_err() || !journal_may_write(&e.path) {
            report.skipped.push((
                paths::JOURNAL.to_owned(),
                format!("refused to write {}", e.path),
            ));
            continue;
        }
        match &e.data {
            Some(d) => storage.write_atomic(&e.path, d.as_bytes())?,
            None => storage.remove(&e.path)?,
        }
    }
    storage.remove(paths::JOURNAL)?;
    Ok(true)
}
