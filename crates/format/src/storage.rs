//! Where library files live. [`DirStorage`] is a folder on disk; [`MemStorage`] keeps files in
//! memory (tests, the web build until browser storage lands) and can simulate a crash at any
//! write. Paths are relative, `/`-separated, and never contain `..`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use crate::StoreError;

/// File access for the library store. Every write is atomic: after a crash the file holds
/// either its old or its new contents, never a mix.
pub trait Storage {
    /// The file's bytes, or `None` if it doesn't exist.
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, StoreError>;
    /// Replaces the file's contents atomically, creating parent folders.
    fn write_atomic(&mut self, path: &str, data: &[u8]) -> Result<(), StoreError>;
    /// Removes the file; removing a missing file is not an error.
    fn remove(&mut self, path: &str) -> Result<(), StoreError>;
    /// Every file under `dir` (recursively), as paths relative to the storage root, sorted.
    fn list(&self, dir: &str) -> Result<Vec<String>, StoreError>;
    /// A description for messages (a folder path).
    fn describe(&self) -> String;
}

/// A boxed storage is a storage, so a session can hold a folder or memory behind one type.
impl Storage for Box<dyn Storage + Send> {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, StoreError> {
        (**self).read(path)
    }
    fn write_atomic(&mut self, path: &str, data: &[u8]) -> Result<(), StoreError> {
        (**self).write_atomic(path, data)
    }
    fn remove(&mut self, path: &str) -> Result<(), StoreError> {
        (**self).remove(path)
    }
    fn list(&self, dir: &str) -> Result<Vec<String>, StoreError> {
        (**self).list(dir)
    }
    fn describe(&self) -> String {
        (**self).describe()
    }
}

/// Rejects absolute paths, `..`, empty components and backslashes, so no record id or file name
/// can reach outside the library.
pub fn check_path(path: &str) -> Result<(), StoreError> {
    let bad = path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|c| c.is_empty() || c == "." || c == "..");
    if bad {
        let shown: String = path.chars().take(80).collect();
        return Err(StoreError::BadPath(format!(
            "`{shown}` is not a library path"
        )));
    }
    Ok(())
}

/// A library folder on disk.
#[derive(Clone, Debug)]
pub struct DirStorage {
    root: PathBuf,
}

impl DirStorage {
    pub fn new(root: impl Into<PathBuf>) -> DirStorage {
        DirStorage { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn full(&self, path: &str) -> Result<PathBuf, StoreError> {
        check_path(path)?;
        Ok(path.split('/').fold(self.root.clone(), |p, c| p.join(c)))
    }
}

fn io(path: &Path, what: &str, e: std::io::Error) -> StoreError {
    StoreError::Io(format!("could not {what} {}: {e}", path.display()))
}

impl Storage for DirStorage {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let full = self.full(path)?;
        match std::fs::read(&full) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(&full, "read", e)),
        }
    }

    fn write_atomic(&mut self, path: &str, data: &[u8]) -> Result<(), StoreError> {
        use std::io::Write;
        let full = self.full(path)?;
        let dir = full
            .parent()
            .ok_or_else(|| StoreError::BadPath(path.to_owned()))?;
        std::fs::create_dir_all(dir).map_err(|e| io(dir, "create", e))?;
        let mut tmp_name = full.file_name().unwrap_or_default().to_os_string();
        tmp_name.push(".tmp");
        let tmp = dir.join(tmp_name);
        {
            let mut f = std::fs::File::create(&tmp).map_err(|e| io(&tmp, "create", e))?;
            f.write_all(data).map_err(|e| io(&tmp, "write", e))?;
            // The data must be on disk before the rename makes it visible.
            f.sync_all().map_err(|e| io(&tmp, "flush", e))?;
        }
        std::fs::rename(&tmp, &full).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            io(&full, "replace", e)
        })?;
        // Persist the rename itself. Directories can't be opened for sync on Windows, where
        // the rename is already durable once it returns, so a failure here is ignored.
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }

    fn remove(&mut self, path: &str) -> Result<(), StoreError> {
        let full = self.full(path)?;
        match std::fs::remove_file(&full) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(&full, "remove", e)),
        }
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, StoreError> {
        let start = self.full(dir)?;
        let mut out = Vec::new();
        // Depth-bounded walk: the library layout is at most four levels deep.
        let mut stack = vec![(start, dir.to_owned(), 0u8)];
        while let Some((path, rel, depth)) = stack.pop() {
            let entries = match std::fs::read_dir(&path) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io(&path, "list", e)),
            };
            for entry in entries.flatten() {
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                let child = format!("{rel}/{name}");
                match entry.file_type() {
                    Ok(t) if t.is_dir() && depth < 8 => {
                        stack.push((entry.path(), child, depth + 1))
                    }
                    Ok(t) if t.is_file() => out.push(child),
                    _ => {}
                }
            }
        }
        out.sort();
        Ok(out)
    }

    fn describe(&self) -> String {
        self.root.display().to_string()
    }
}

#[derive(Debug, Default)]
struct MemInner {
    files: BTreeMap<String, Vec<u8>>,
    /// Mutating calls left before the simulated crash; `None` never crashes.
    writes_left: Option<usize>,
    writes: usize,
    /// Paths written or removed since the last [`MemStorage::take_changes`], when tracking.
    changes: Option<std::collections::BTreeSet<String>>,
}

/// Files in memory. Clones share the same files, so a test can "crash" one handle and reopen the
/// library through another.
#[derive(Clone, Debug, Default)]
pub struct MemStorage {
    inner: Arc<Mutex<MemInner>>,
}

impl MemStorage {
    pub fn new() -> MemStorage {
        MemStorage::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MemInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// After `n` more successful writes or removes, every later one fails as if the machine lost
    /// power: the write in progress leaves only a stray temp file. `None` turns crashing off.
    pub fn crash_after(&self, n: Option<usize>) {
        self.lock().writes_left = n;
    }

    /// Writes and removes done so far.
    pub fn write_count(&self) -> usize {
        self.lock().writes
    }

    /// Starts noting which files change, so a copy elsewhere (the browser's IndexedDB) can be
    /// kept up to date with [`MemStorage::take_changes`].
    pub fn track_changes(&self) {
        let mut inner = self.lock();
        if inner.changes.is_none() {
            inner.changes = Some(std::collections::BTreeSet::new());
        }
    }

    /// The files changed since the last call: each path with its bytes now, or `None` if it was
    /// removed. Empty unless [`MemStorage::track_changes`] was called.
    pub fn take_changes(&self) -> Vec<(String, Option<Vec<u8>>)> {
        let mut inner = self.lock();
        let paths = match inner.changes.as_mut() {
            Some(c) => std::mem::take(c),
            None => return Vec::new(),
        };
        paths
            .into_iter()
            .map(|p| {
                let data = inner.files.get(&p).cloned();
                (p, data)
            })
            .collect()
    }

    /// A snapshot of every file.
    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        self.lock().files.clone()
    }

    /// Overwrites a file directly, bypassing atomicity (to plant damage in tests).
    pub fn plant(&self, path: &str, data: &[u8]) {
        self.lock().files.insert(path.to_owned(), data.to_vec());
    }

    fn gate(inner: &mut MemInner, path: &str) -> Result<(), StoreError> {
        match inner.writes_left {
            Some(0) => {
                inner
                    .files
                    .insert(format!("{path}.tmp"), b"torn write".to_vec());
                Err(StoreError::Io(format!("simulated crash writing {path}")))
            }
            Some(n) => {
                inner.writes_left = Some(n - 1);
                inner.writes += 1;
                Ok(())
            }
            None => {
                inner.writes += 1;
                Ok(())
            }
        }
    }
}

impl Storage for MemStorage {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, StoreError> {
        check_path(path)?;
        Ok(self.lock().files.get(path).cloned())
    }

    fn write_atomic(&mut self, path: &str, data: &[u8]) -> Result<(), StoreError> {
        check_path(path)?;
        let mut inner = self.lock();
        MemStorage::gate(&mut inner, path)?;
        inner.files.insert(path.to_owned(), data.to_vec());
        if let Some(c) = inner.changes.as_mut() {
            c.insert(path.to_owned());
        }
        Ok(())
    }

    fn remove(&mut self, path: &str) -> Result<(), StoreError> {
        check_path(path)?;
        let mut inner = self.lock();
        MemStorage::gate(&mut inner, path)?;
        inner.files.remove(path);
        if let Some(c) = inner.changes.as_mut() {
            c.insert(path.to_owned());
        }
        Ok(())
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, StoreError> {
        check_path(dir)?;
        let prefix = format!("{dir}/");
        Ok(self
            .lock()
            .files
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect())
    }

    fn describe(&self) -> String {
        "memory".to_owned()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn memory_tracks_changes_for_a_copy_elsewhere() {
        let mut m = MemStorage::new();
        m.write_atomic("a.json", b"1").unwrap();
        assert!(m.take_changes().is_empty(), "not tracking yet");
        m.track_changes();
        m.write_atomic("a.json", b"2").unwrap();
        m.write_atomic("b/c.json", b"3").unwrap();
        m.remove("b/c.json").unwrap();
        assert_eq!(
            m.take_changes(),
            vec![
                ("a.json".to_owned(), Some(b"2".to_vec())),
                ("b/c.json".to_owned(), None)
            ]
        );
        assert!(m.take_changes().is_empty());
    }

    use super::*;

    #[test]
    fn paths_cannot_escape() {
        for bad in [
            "",
            "/etc/passwd",
            "a/../b",
            "a//b",
            "./a",
            "a\\b",
            "C:/x",
            "a/",
        ] {
            assert!(check_path(bad).is_err(), "{bad}");
        }
        assert!(check_path("scores/abc.json").is_ok());
    }

    #[test]
    fn dir_storage_round_trip() {
        let dir = std::env::temp_dir().join(format!("scorecraft-format-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = DirStorage::new(&dir);
        s.write_atomic("a/b/c.json", b"one").unwrap();
        s.write_atomic("a/b/c.json", b"two").unwrap();
        s.write_atomic("a/d.json", b"three").unwrap();
        assert_eq!(s.read("a/b/c.json").unwrap().as_deref(), Some(&b"two"[..]));
        assert_eq!(s.read("a/missing").unwrap(), None);
        assert_eq!(s.list("a").unwrap(), vec!["a/b/c.json", "a/d.json"]);
        assert!(s.list("nothing").unwrap().is_empty());
        s.remove("a/d.json").unwrap();
        s.remove("a/d.json").unwrap();
        assert_eq!(s.list("a").unwrap(), vec!["a/b/c.json"]);
        assert!(s.read("../x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mem_storage_crashes_on_cue() {
        let mut m = MemStorage::new();
        m.crash_after(Some(1));
        m.write_atomic("x", b"1").unwrap();
        assert!(m.write_atomic("y", b"2").is_err());
        assert!(m.remove("x").is_err());
        assert_eq!(m.read("x").unwrap().as_deref(), Some(&b"1"[..]));
        assert_eq!(
            m.read("y.tmp").unwrap().as_deref(),
            Some(&b"torn write"[..])
        );
    }
}
