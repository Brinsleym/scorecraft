//! Sync (feature-map SYNC-1, SYNC-3, SYNC-6, SYNC-7; spec `docs/spec/sync.md`): the library's
//! replica, its change log, and the shared-folder backend.
//!
//! Every library edit already saves the records it touched; with sync on, those records are
//! compared with the replica's copy after each command and the differences logged as changes
//! (`scorecraft-sync`), so no command needs sync code of its own. The log lives in the library
//! under `sync/log/` as append-only blocks; the replica is rebuilt from it when the library
//! opens.
//!
//! The shared-folder backend is any folder every device can reach: a folder Google Drive,
//! OneDrive, Dropbox, iCloud Drive or Syncthing keeps in sync, or a network share. Each device
//! writes only new files there, never changing one, so the cloud tools never see a conflict:
//!
//! ```text
//! <folder>/scorecraft-sync.json                    marker
//! <folder>/changes/<device>/<first>-<last>.json    a block of one device's changes
//! <folder>/blobs/<sha-256>                         score files, by content
//! ```

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use scorecraft_doc::{BlobRef, DeviceId, Hlc, Library};
use scorecraft_format::{DirStorage, Storage, Store};
use scorecraft_ops::{History, Scope, Touched};
use scorecraft_sync::{Change, Record, Replica, Summary};

use crate::commands::{CommandSpec, bad, opt_str, params_object, reject_unknown_keys, req_str};
use crate::library::{AnyStorage, LibrarySession, library_open};
use crate::{EngineError, Session};

/// Most problems listed by `sync.status`.
const MAX_PROBLEMS: usize = 20;

/// Sync's state for an open library.
pub(crate) struct SyncState {
    pub(crate) replica: Replica,
    backend: Option<Backend>,
    /// The next local log block's number.
    next_block: u64,
    /// Changes recorded but not yet written to the local log (a write failed); retried.
    unsaved: Vec<Change>,
    last_sync_ms: Option<u64>,
    /// Score files the last sync could find on neither side.
    waiting_files: usize,
    problems: Vec<String>,
}

struct Backend {
    storage: AnyStorage,
    label: String,
}

#[derive(Serialize, Deserialize)]
struct DeviceFile {
    device: DeviceId,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum BackendFile {
    None,
    Folder { path: String },
}

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, menu, params, ours, run| CommandSpec {
        id,
        label,
        menu,
        shortcut: None,
        params,
        ours,
        enabled: library_open,
        run,
    };
    vec![
        spec(
            "sync.backend.set",
            "Sync Through a Folder…",
            "Library",
            r#"{"kind":"folder"|"none","path":str?}"#,
            false,
            backend_set as _,
        ),
        spec("sync.now", "Sync Now", "Library", "{}", false, now as _),
        spec("sync.status", "Sync Status", "", "{}", false, status as _),
    ]
}

fn record_of(t: Touched) -> Record {
    match t {
        Touched::Score(id) => Record::Score(id),
        Touched::List(id) => Record::List(id),
        Touched::Library => Record::Library,
    }
}

fn touched_of(r: Record) -> Touched {
    match r {
        Record::Score(id) => Touched::Score(id),
        Record::List(id) => Touched::List(id),
        Record::Library => Touched::Library,
    }
}

fn block_name(n: u64) -> String {
    format!("log/{n:010}.json")
}

/// `changes/<device>/<first>-<last>.json` → (device, first, last).
fn parse_block(path: &str) -> Option<(DeviceId, u64, u64)> {
    let rest = path.strip_prefix("changes/")?;
    let (dev, file) = rest.split_once('/')?;
    let (first, last) = file.strip_suffix(".json")?.split_once('-')?;
    Some((dev.parse().ok()?, first.parse().ok()?, last.parse().ok()?))
}

impl SyncState {
    /// Append `changes` to the library's local log (and anything a failed write left).
    fn append(&mut self, store: &mut Store<AnyStorage>, changes: &[Change]) {
        self.unsaved.extend_from_slice(changes);
        if self.unsaved.is_empty() {
            return;
        }
        let data = match serde_json::to_vec(&self.unsaved) {
            Ok(d) => d,
            Err(e) => return self.problem(format!("the sync log could not be written: {e}")),
        };
        match store.set_sync_file(&block_name(self.next_block), &data) {
            Ok(()) => {
                self.next_block += 1;
                self.unsaved.clear();
            }
            Err(e) => self.problem(format!("the sync log could not be written: {e}")),
        }
    }

    fn problem(&mut self, p: String) {
        if self.problems.len() >= MAX_PROBLEMS {
            self.problems.remove(0);
        }
        self.problems.push(p);
    }
}

fn open_backend(file: &BackendFile) -> Option<Backend> {
    match file {
        BackendFile::None => None,
        BackendFile::Folder { path } => Some(Backend {
            storage: Box::new(DirStorage::new(path)),
            label: path.clone(),
        }),
    }
}

/// Sync's state for a library that has it on (it has `sync/device.json`), rebuilt from the log;
/// edits made while sync wasn't recording (an older build, a crash) are recorded now.
pub(crate) fn load(
    store: &mut Store<AnyStorage>,
    lib: &Library,
    now_ms: u64,
) -> Result<Option<SyncState>, EngineError> {
    let Some(raw) = store.sync_file("device.json")? else {
        return Ok(None);
    };
    let device: DeviceFile = serde_json::from_slice(&raw)
        .map_err(|e| EngineError::Io(format!("sync/device.json is damaged: {e}")))?;
    let mut st = SyncState {
        replica: Replica::new(device.device, Library::new(lib.id)),
        backend: None,
        next_block: 0,
        unsaved: Vec::new(),
        last_sync_ms: None,
        waiting_files: 0,
        problems: Vec::new(),
    };
    for name in store.sync_files("log")? {
        let n = name
            .strip_prefix("log/")
            .and_then(|f| f.strip_suffix(".json"))
            .and_then(|f| f.parse::<u64>().ok());
        let Some(n) = n else { continue };
        st.next_block = st.next_block.max(n + 1);
        let parsed = store
            .sync_file(&name)?
            .map(|d| serde_json::from_slice::<Vec<Change>>(&d));
        match parsed {
            Some(Ok(changes)) => {
                st.replica.receive(&changes, now_ms);
            }
            Some(Err(e)) => st.problem(format!("sync/{name} could not be read: {e}")),
            None => {}
        }
    }
    if let Some(raw) = store.sync_file("backend.json")? {
        match serde_json::from_slice::<BackendFile>(&raw) {
            Ok(f) => st.backend = open_backend(&f),
            Err(e) => st.problem(format!("sync/backend.json could not be read: {e}")),
        }
    }
    let caught_up = st.replica.record_all(lib, now_ms);
    st.append(store, &caught_up);
    Ok(Some(st))
}

impl LibrarySession {
    /// Turn sync on for this library: give this device an id and record the whole library.
    fn enable_sync(&mut self, now_ms: u64) -> Result<(), EngineError> {
        if self.sync.is_some() {
            return Ok(());
        }
        let device: DeviceId = self.ids.id(now_ms);
        let data = serde_json::to_vec(&DeviceFile { device })
            .map_err(|e| EngineError::Io(format!("sync/device.json: {e}")))?;
        self.store.set_sync_file("device.json", &data)?;
        let mut st = SyncState {
            replica: Replica::new(device, Library::new(self.lib.id)),
            backend: None,
            next_block: 0,
            unsaved: Vec::new(),
            last_sync_ms: None,
            waiting_files: 0,
            problems: Vec::new(),
        };
        let all = st.replica.record_all(&self.lib, now_ms);
        st.append(&mut self.store, &all);
        self.hlc = Hlc::new(self.hlc.ms, self.hlc.counter, device);
        self.sync = Some(st);
        Ok(())
    }

    fn set_backend(
        &mut self,
        file: &BackendFile,
        backend: Option<Backend>,
    ) -> Result<(), EngineError> {
        let data = serde_json::to_vec(file)
            .map_err(|e| EngineError::Io(format!("sync/backend.json: {e}")))?;
        self.store.set_sync_file("backend.json", &data)?;
        if let Some(st) = self.sync.as_mut() {
            st.backend = backend;
        }
        Ok(())
    }
}

impl Session {
    /// Record what the last command changed as sync changes (called after every command).
    pub(crate) fn record_sync(&mut self) {
        let now = self.now_ms();
        let Some(l) = self.library.as_mut() else {
            return;
        };
        if l.dirty.is_empty() {
            return;
        }
        let records: Vec<Record> = std::mem::take(&mut l.dirty)
            .into_iter()
            .map(record_of)
            .collect();
        let Some(st) = l.sync.as_mut() else {
            return;
        };
        let changes = st.replica.record(&l.lib, &records, now);
        st.append(&mut l.store, &changes);
    }

    /// Sync through `storage` (tests, and shells with their own folder access). Turns sync on
    /// if it is off; not remembered when the library reopens.
    pub fn set_sync_storage(
        &mut self,
        storage: AnyStorage,
        label: &str,
    ) -> Result<(), EngineError> {
        let now = self.now_ms();
        let l = self.lib_mut("sync.backend.set")?;
        l.enable_sync(now)?;
        if let Some(st) = l.sync.as_mut() {
            st.backend = Some(Backend {
                storage,
                label: label.to_owned(),
            });
        }
        Ok(())
    }
}

fn backend_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "sync.backend.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["kind", "path"])?;
    let kind = req_str(ID, p, "kind")?;
    let path = opt_str(ID, p, "path")?;
    let now = session.now_ms();
    let file = match (kind, path) {
        ("none", None) => BackendFile::None,
        ("folder", Some(path)) => {
            let shown = crate::commands::truncate(path, 256);
            if path.trim().is_empty() {
                return Err(bad(ID, "`path` must name a folder".into()));
            }
            match std::fs::metadata(path) {
                Ok(m) if m.is_dir() => {}
                Ok(_) => return Err(EngineError::Io(format!("{shown} is not a folder"))),
                Err(e) => return Err(EngineError::Io(format!("{shown}: {e}"))),
            }
            BackendFile::Folder {
                path: path.to_owned(),
            }
        }
        ("folder", None) => return Err(bad(ID, "a folder backend needs `path`".into())),
        ("none", Some(_)) => return Err(bad(ID, "`path` goes with kind `folder`".into())),
        (other, _) => {
            return Err(bad(
                ID,
                format!(
                    "unknown kind `{}`: folder or none",
                    crate::commands::truncate(other, 32)
                ),
            ));
        }
    };
    let l = session.lib_mut(ID)?;
    if matches!(file, BackendFile::Folder { .. }) {
        l.enable_sync(now)?;
    }
    let backend = open_backend(&file);
    l.set_backend(&file, backend)?;
    Ok(status_json(l))
}

fn status_json(l: &LibrarySession) -> Value {
    match &l.sync {
        None => json!({ "on": false }),
        Some(st) => json!({
            "on": true,
            "device": st.replica.device().to_string(),
            "backend": st.backend.as_ref().map(|b| b.label.clone()),
            "changes": st.replica.log_len(),
            "pending": st.replica.pending(),
            "lastSyncMs": st.last_sync_ms,
            "waitingFiles": st.waiting_files,
            "problems": st.problems,
        }),
    }
}

fn status(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let p = params_object("sync.status", params)?;
    reject_unknown_keys("sync.status", p, &[])?;
    let l = session.lib_mut("sync.status")?;
    Ok(status_json(l))
}

fn io(e: scorecraft_format::StoreError) -> EngineError {
    EngineError::Io(format!("the sync folder: {e}"))
}

fn now(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "sync.now";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &[])?;
    session.record_sync();
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let LibrarySession {
        store,
        lib,
        history,
        sync,
        ..
    } = l;
    let Some(st) = sync.as_mut() else {
        return Err(EngineError::Io(
            "sync is off: choose a folder with sync.backend.set".into(),
        ));
    };
    let Some(mut backend) = st.backend.take() else {
        return Err(EngineError::Io(
            "sync has no folder: choose one with sync.backend.set".into(),
        ));
    };
    let out = sync_with(st, &mut backend.storage, store, lib, history, now);
    st.backend = Some(backend);
    out
}

/// One sync with the folder `fs`: send, receive, then score files.
fn sync_with(
    st: &mut SyncState,
    fs: &mut AnyStorage,
    store: &mut Store<AnyStorage>,
    lib: &mut Library,
    history: &mut History,
    now: u64,
) -> Result<Value, EngineError> {
    if fs.read("scorecraft-sync.json").map_err(io)?.is_none() {
        let marker = json!({ "format": "scorecraft-sync", "version": 1 }).to_string();
        fs.write_atomic("scorecraft-sync.json", marker.as_bytes())
            .map_err(io)?;
    }
    let me = st.replica.device();

    // Send: this device's changes the folder doesn't have yet.
    let blocks: Vec<(DeviceId, u64, u64, String)> = fs
        .list("changes")
        .map_err(io)?
        .into_iter()
        .filter_map(|p| parse_block(&p).map(|(d, a, b)| (d, a, b, p)))
        .collect();
    let written = blocks
        .iter()
        .filter(|b| b.0 == me)
        .map(|b| b.2)
        .max()
        .unwrap_or(0);
    let mut only_mine: Summary = st
        .replica
        .summary()
        .keys()
        .map(|d| (*d, u64::MAX))
        .collect();
    only_mine.insert(me, written);
    let mine: Vec<Change> = st
        .replica
        .changes_for(&only_mine)
        .into_iter()
        .filter(|c| c.id.device == me)
        .collect();
    let mut sent = 0;
    if let (Some(first), Some(last)) = (mine.first(), mine.last()) {
        let path = format!("changes/{me}/{:020}-{:020}.json", first.id.seq, last.id.seq);
        let data =
            serde_json::to_vec(&mine).map_err(|e| EngineError::Io(format!("sync changes: {e}")))?;
        fs.write_atomic(&path, &data).map_err(io)?;
        sent = mine.len();
    }

    // Receive: other devices' blocks with changes this replica hasn't seen.
    let have = st.replica.summary();
    let mut incoming = Vec::new();
    let mut problems = Vec::new();
    for (dev, _, last, path) in &blocks {
        if *dev == me || have.get(dev).is_some_and(|h| h >= last) {
            continue;
        }
        let parsed = fs
            .read(path)
            .map_err(io)?
            .map(|d| serde_json::from_slice::<Vec<Change>>(&d));
        match parsed {
            Some(Ok(changes)) => incoming.extend(
                changes
                    .into_iter()
                    // A block speaks only for its own device.
                    .filter(|c| c.id.device == *dev && !st.replica.knows(&c.id)),
            ),
            Some(Err(e)) => problems.push(format!("{path} could not be read: {e}")),
            None => {}
        }
    }
    let received = st.replica.receive(&incoming, now);
    st.replica.copy_into(lib, &received.changed);
    let touched: BTreeSet<Touched> = received.changed.iter().map(|r| touched_of(*r)).collect();
    store.save(lib, &touched)?;
    // Undo steps from before can't be trusted over what arrived.
    for t in &touched {
        history.forget(match t {
            Touched::Score(id) => Scope::Score(*id),
            _ => Scope::Library,
        });
    }
    st.append(store, &incoming);

    // Score files: send ours the folder lacks, fetch the ones we lack.
    let theirs: BTreeSet<String> = fs.list("blobs").map_err(io)?.into_iter().collect();
    let mut wanted: BTreeMap<String, BlobRef> = BTreeMap::new();
    for s in lib.scores.values() {
        let more = s.source.more.iter().copied();
        let audio = s.audio.iter().map(|a| a.blob);
        for b in std::iter::once(s.source.blob).chain(more).chain(audio) {
            wanted.insert(format!("blobs/{}", b.to_hex()), b);
        }
    }
    let (mut uploaded, mut downloaded, mut waiting) = (0, 0, 0);
    for (path, b) in &wanted {
        let here = store.has_blob(*b)?;
        let there = theirs.contains(path);
        if here && !there {
            fs.write_atomic(path, &store.blob(*b)?).map_err(io)?;
            uploaded += 1;
        } else if !here && there {
            match fs.read(path).map_err(io)? {
                Some(data) if store.put_blob(&data)? == *b => downloaded += 1,
                Some(_) => problems.push(format!("{path} does not match its name; skipped")),
                None => waiting += 1,
            }
        } else if !here {
            waiting += 1;
        }
    }
    st.last_sync_ms = Some(now);
    st.waiting_files = waiting;
    for p in problems {
        st.problem(p);
    }
    Ok(json!({
        "sent": sent,
        "received": received.new,
        "changed": received.changed.len(),
        "uploaded": uploaded,
        "downloaded": downloaded,
        "waitingFiles": waiting,
        "pending": st.replica.pending(),
    }))
}

#[cfg(test)]
mod tests;
