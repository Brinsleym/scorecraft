//! The open library: the model, its store, its undo history and the id generator. Every edit
//! command goes through [`LibrarySession::commit`], which applies one transaction, records one
//! undo step and saves the records it touched before returning.

use std::collections::BTreeSet;

use scorecraft_doc::{DeviceId, Hlc, IdGen, Library};
use scorecraft_format::{DirStorage, MemStorage, Storage, Store};
use scorecraft_ops::{Applied, History, Scope, Touched, Transaction, Undone};

use crate::{EngineError, Session};

/// The storage a session's library lives in: a folder, or memory for tests and the web build.
pub type AnyStorage = Box<dyn Storage + Send>;

pub(crate) struct LibrarySession {
    pub(crate) store: Store<AnyStorage>,
    pub(crate) lib: Library,
    pub(crate) history: History,
    pub(crate) ids: IdGen,
    /// Where it lives, for messages and `library.info`.
    pub(crate) location: String,
    /// Records that could not be read when it opened, for `library.verify`.
    pub(crate) unreadable: Vec<String>,
    /// This session's hybrid logical clock; every marking is stamped with it (architecture §9).
    pub(crate) hlc: Hlc,
}

impl LibrarySession {
    /// Applies `tx` as one undo step and saves what it touched. If the save fails, the change is
    /// undone in memory too, so memory and disk never disagree, and the error is returned.
    pub(crate) fn commit(&mut self, label: &str, tx: Transaction) -> Result<Applied, EngineError> {
        let applied = self.history.commit(&mut self.lib, label, tx)?;
        if applied.inverse.is_empty() {
            return Ok(applied);
        }
        if let Err(e) = self.store.save(&self.lib, &applied.touched) {
            let scope = Scope::of(&applied.touched);
            let _ = self.history.undo(&mut self.lib, Some(scope));
            // The undone step sits on the redo stack; it never reached disk, so drop it.
            self.history.forget_redo(scope);
            return Err(e.into());
        }
        Ok(applied)
    }

    /// Saves a change that is not an undo step (the score was opened).
    pub(crate) fn save_untracked(&mut self, touched: Touched) -> Result<(), EngineError> {
        self.store.save(&self.lib, &BTreeSet::from([touched]))?;
        Ok(())
    }

    pub(crate) fn save_undone(&mut self, u: &Undone) -> Result<(), EngineError> {
        self.store.save(&self.lib, &u.touched)?;
        Ok(())
    }
}

/// The wall clock in Unix milliseconds, through `chrono` (rule 10). The browser build has no
/// system clock in Rust: its shell sets one with [`Session::set_clock`]; until then ids still
/// come out unique and ordered (the id generator counts within a millisecond).
#[cfg(not(target_arch = "wasm32"))]
fn system_ms() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
fn system_ms() -> u64 {
    0
}

/// A clock a frontend or test can supply.
pub type Clock = Box<dyn Fn() -> u64 + Send>;

pub(crate) fn default_clock() -> Clock {
    Box::new(system_ms)
}

/// Seeds the id generator from the clock and, natively, the process id. Ids need uniqueness,
/// not secrecy; the v7 time prefix and per-millisecond counter carry most of it.
fn seed(now_ms: u64, salt: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |b: u8| {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    for b in now_ms.to_le_bytes() {
        mix(b);
    }
    for b in salt.bytes() {
        mix(b);
    }
    #[cfg(not(target_arch = "wasm32"))]
    for b in std::process::id().to_le_bytes() {
        mix(b);
    }
    #[cfg(not(target_arch = "wasm32"))]
    for b in chrono::Utc::now().timestamp_subsec_nanos().to_le_bytes() {
        mix(b);
    }
    h
}

impl Session {
    /// Replaces the wall clock (Unix milliseconds). The web shell passes `Date.now`; tests pass a
    /// fixed clock.
    pub fn set_clock(&mut self, clock: Clock) {
        self.clock = clock;
    }

    pub(crate) fn now_ms(&self) -> u64 {
        (self.clock)()
    }

    /// Whether a library is open.
    pub fn has_library(&self) -> bool {
        self.library.is_some()
    }

    /// Creates a library in `storage` and makes it the session's library.
    pub fn create_library_in(
        &mut self,
        storage: AnyStorage,
        location: &str,
    ) -> Result<(), EngineError> {
        let now = self.now_ms();
        let mut ids = IdGen::new(seed(now, location));
        let id = ids.id(now);
        let (store, lib) = Store::create(storage, id, now)?;
        self.attach(store, lib, ids, location, Vec::new());
        Ok(())
    }

    /// Opens the library in `storage`. Returns the records that could not be read.
    pub fn open_library_in(
        &mut self,
        storage: AnyStorage,
        location: &str,
    ) -> Result<Vec<(String, String)>, EngineError> {
        let now = self.now_ms();
        let (store, lib, report) = Store::open(storage)?;
        let ids = IdGen::new(seed(now, location));
        let unreadable = report.skipped.iter().map(|(p, _)| p.clone()).collect();
        self.attach(store, lib, ids, location, unreadable);
        Ok(report.skipped)
    }

    /// A new library in memory (tests, the web build until browser storage lands).
    pub fn create_memory_library(&mut self) -> Result<MemStorage, EngineError> {
        let mem = MemStorage::new();
        self.create_library_in(Box::new(mem.clone()), "memory")?;
        Ok(mem)
    }

    /// Creates a library folder at `path`. The folder must not exist yet, or be empty, so a
    /// mistyped path never scatters library files into a folder of other things.
    pub(crate) fn create_library_dir(&mut self, path: &str) -> Result<(), EngineError> {
        let shown = crate::commands::truncate(path, 256);
        if path.trim().is_empty() {
            return Err(EngineError::Io("the library needs a folder path".into()));
        }
        match std::fs::symlink_metadata(path) {
            Ok(m) if !m.is_dir() => {
                return Err(EngineError::Io(format!(
                    "{shown} exists and is not a folder"
                )));
            }
            Ok(_) => {
                let empty = std::fs::read_dir(path)
                    .map_err(|e| EngineError::Io(format!("{shown}: {e}")))?
                    .next()
                    .is_none();
                if !empty {
                    let is_lib = std::path::Path::new(path).join("manifest.json").exists();
                    return Err(if is_lib {
                        scorecraft_format::StoreError::Exists(shown).into()
                    } else {
                        EngineError::Io(format!(
                            "{shown} is a folder with other files in it; choose a new or empty folder"
                        ))
                    });
                }
            }
            Err(_) => {}
        }
        self.create_library_in(Box::new(DirStorage::new(path)), path)
    }

    pub(crate) fn open_library_dir(
        &mut self,
        path: &str,
    ) -> Result<Vec<(String, String)>, EngineError> {
        self.open_library_in(Box::new(DirStorage::new(path)), path)
    }

    fn attach(
        &mut self,
        store: Store<AnyStorage>,
        lib: Library,
        mut ids: IdGen,
        location: &str,
        unreadable: Vec<String>,
    ) {
        // A device id per session until device identity is stored with sync (M8).
        let now = self.now_ms();
        let device: DeviceId = ids.id(now);
        self.library = Some(LibrarySession {
            store,
            lib,
            history: History::default(),
            ids,
            location: location.to_owned(),
            unreadable,
            hlc: Hlc::new(0, 0, device),
        });
    }

    /// The open library's model (read-only), for frontends and tests.
    pub fn library(&self) -> Option<&Library> {
        self.library.as_ref().map(|l| &l.lib)
    }

    pub(crate) fn lib_mut(&mut self, command: &str) -> Result<&mut LibrarySession, EngineError> {
        self.library
            .as_mut()
            .ok_or_else(|| EngineError::Disabled(command.to_owned()))
    }
}

/// Enabled while a library is open.
pub fn library_open(session: &Session) -> bool {
    session.has_library()
}
