//! Persistent configuration storage (Settings, keyboard shortcut presets, the crash-recovery
//! sentinel). The engine only sees the [`ConfigStore`] trait; frontends choose where it lives:
//! the desktop app uses a [`DirConfig`] in the platform config directory, the web app backs it
//! with the browser's Origin Private File System (IndexedDB fallback), tests and headless runs use
//! [`MemoryConfig`] (or none at all).
//!
//! Auto-saves are project files, not settings, but they live wherever the store says: the store's
//! [`FileOps`] (the file system by default; the browser's storage on the web).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Named text blobs (`prefs.json`, `shortcuts.json`, `session.lock`).
pub trait ConfigStore: Send + Sync {
    fn read(&self, name: &str) -> Option<String>;
    /// Read a bounded UTF-8 entry, distinguishing missing entries from read errors.
    /// The compatible default checks an existing store's returned string. Stores that read
    /// from disk should override this to enforce the limit while reading.
    fn read_bounded(&self, name: &str, max_bytes: usize) -> std::io::Result<Option<String>> {
        let text = self.read(name);
        if text.as_ref().is_some_and(|text| text.len() > max_bytes) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "configuration entry exceeds its size limit"));
        }
        Ok(text)
    }
    fn write(&self, name: &str, data: &str) -> std::io::Result<()>;
    fn remove(&self, name: &str) -> std::io::Result<()>;
    /// Names of the entries directly in `dir` (a `/`-separated name prefix such as `Scripts`),
    /// sorted. Stores that can't list return nothing.
    fn list(&self, dir: &str) -> Vec<String> {
        let _ = dir;
        vec![]
    }
    /// The folder next to the configuration (default Auto-Save folder for untitled projects),
    /// in the namespace of [`ConfigStore::files`]; `None` when there is none.
    fn dir(&self) -> Option<PathBuf> {
        None
    }
    /// Where auto-saves are listed, written and found (default: the file system).
    fn files(&self) -> &dyn FileOps {
        &StdFiles
    }
}

/// The file operations auto-save and crash recovery need.
pub trait FileOps: Send + Sync {
    /// Files directly in `dir`: (file name, modified time).
    fn list(&self, dir: &Path) -> Vec<(String, std::time::SystemTime)>;
    /// Write `data` to `path` atomically, creating its folder.
    fn write(&self, path: &Path, data: &[u8]) -> std::io::Result<()>;
    fn remove(&self, path: &Path) -> std::io::Result<()>;
    fn is_file(&self, path: &Path) -> bool;
}

/// [`FileOps`] on the file system.
pub struct StdFiles;

impl FileOps for StdFiles {
    fn list(&self, dir: &Path) -> Vec<(String, std::time::SystemTime)> {
        let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
        rd.flatten()
            .filter_map(|e| {
                let m = e.metadata().ok().filter(|m| m.is_file())?.modified().ok()?;
                Some((e.file_name().to_string_lossy().to_string(), m))
            })
            .collect()
    }
    fn write(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
        if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d)?;
        }
        atomic_write(path, data)
    }
    fn remove(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_file(path)
    }
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}

/// In-memory store (tests, headless sessions).
#[derive(Default)]
pub struct MemoryConfig {
    pub files: Mutex<BTreeMap<String, String>>,
}

impl ConfigStore for MemoryConfig {
    fn read(&self, name: &str) -> Option<String> {
        self.files.lock().ok()?.get(name).cloned()
    }
    fn write(&self, name: &str, data: &str) -> std::io::Result<()> {
        if let Ok(mut m) = self.files.lock() {
            m.insert(name.to_string(), data.to_string());
        }
        Ok(())
    }
    fn remove(&self, name: &str) -> std::io::Result<()> {
        if let Ok(mut m) = self.files.lock() {
            m.remove(name);
        }
        Ok(())
    }
    fn list(&self, dir: &str) -> Vec<String> {
        let pre = format!("{}/", dir.trim_end_matches('/'));
        let Ok(m) = self.files.lock() else { return vec![] };
        m.keys().filter_map(|k| k.strip_prefix(&pre)).filter(|n| !n.is_empty() && !n.contains('/')).map(str::to_string).collect()
    }
}

/// Files in one directory, written atomically.
pub struct DirConfig {
    pub dir: PathBuf,
}

impl DirConfig {
    pub fn new(dir: impl Into<PathBuf>) -> DirConfig {
        DirConfig { dir: dir.into() }
    }
}

impl ConfigStore for DirConfig {
    fn read(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.dir.join(name)).ok()
    }
    fn read_bounded(&self, name: &str, max_bytes: usize) -> std::io::Result<Option<String>> {
        use std::io::Read;
        let limit = u64::try_from(max_bytes)
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "configuration read limit is too large"))?;
        let file = match std::fs::File::open(self.dir.join(name)) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut text = String::new();
        file.take(limit).read_to_string(&mut text)?;
        if text.len() > max_bytes {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "configuration entry exceeds its size limit"));
        }
        Ok(Some(text))
    }
    fn write(&self, name: &str, data: &str) -> std::io::Result<()> {
        let path = self.dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap_or(&self.dir))?;
        atomic_write(&path, data.as_bytes())
    }
    fn list(&self, dir: &str) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(self.dir.join(dir)) else { return vec![] };
        let mut v: Vec<String> = rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        v.sort();
        v
    }
    fn remove(&self, name: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.dir.join(name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
    fn dir(&self) -> Option<PathBuf> {
        Some(self.dir.clone())
    }
}

/// The temporary file an atomic write of `path` goes through.
pub fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Write `data` to `path` atomically: write a temporary file next to it, flush it to disk, then
/// rename it over `path`. A crash at any point leaves either the old file or the new one, never a
/// torn file.
pub fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    atomic_write_with(path, data, |tmp, data| {
        use std::io::Write;
        let mut f = std::fs::File::create(tmp)?;
        f.write_all(data)?;
        f.sync_all()
    })
}

/// [`atomic_write`] with an injectable temp-file writer (tests simulate a crash mid-write). A
/// failed write removes the temporary file and leaves `path` untouched.
pub fn atomic_write_with(path: &Path, data: &[u8], write: impl FnOnce(&Path, &[u8]) -> std::io::Result<()>) -> std::io::Result<()> {
    let tmp = temp_path(path);
    if let Err(e) = write(&tmp, data) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, path)
}

/// Publish a fully flushed new file without replacing an existing file or symlink.
/// A same-directory hard link atomically claims the destination only after staging succeeds.
/// Filesystems without hard-link support fail closed; there is no unsafe rename fallback.
pub fn atomic_write_new(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    for _ in 0..32 {
        let tmp = parent.join(format!(".unwritten-copy-{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let result = (|| {
            file.write_all(data)?;
            file.sync_all()?;
            std::fs::hard_link(&tmp, path)
        })();
        drop(file);
        // Never remove the destination on failure: it may belong to another writer.
        let _ = std::fs::remove_file(&tmp);
        return result;
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "could not reserve a staging file for the new copy"))
}

#[cfg(test)]
mod new_file_tests {
    use super::atomic_write_new;

    #[test]
    fn concurrent_publish_has_exactly_one_winner_and_leaves_no_staging_files() {
        let dir = std::env::temp_dir().join(format!("unwritten-new-file-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("copy.aep");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let handles: Vec<_> = (0..8u8)
            .map(|n| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let data = vec![n; 8192];
                    (n, atomic_write_new(&path, &data))
                })
            })
            .collect();
        let mut winners = Vec::new();
        for handle in handles {
            let (n, result) = handle.join().unwrap();
            match result {
                Ok(()) => winners.push(n),
                Err(e) => assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists),
            }
        }
        assert_eq!(winners.len(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), vec![winners[0]; 8192]);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        assert_eq!(atomic_write_new(&path, b"replacement").unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).unwrap(), vec![winners[0]; 8192]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_not_replaced_or_followed() {
        let dir = std::env::temp_dir().join(format!("unwritten-new-file-symlink-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let target = dir.join("absent-original.aep");
        let path = dir.join("copy.aep");
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert_eq!(atomic_write_new(&path, b"copy").unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_link(&path).unwrap(), target);
        assert!(!target.exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
