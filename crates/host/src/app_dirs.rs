//! Where Unwritten Studio keeps its per-user data (settings, shortcuts, crash recovery, logs, models,
//! plug-ins): the one directory [`config_dir`](crate::config_dir) returns.
//!
//! In order:
//! 1. `UNWRITTEN_STUDIO_CONFIG_DIR`, when set (tests, agents, custom setups).
//! 2. **Portable mode** (#993): a marker file ([`PORTABLE_MARKERS`]) beside the executable puts
//!    everything in `<exe dir>/Unwritten StudioData`, so a portable copy on a USB stick leaves nothing in
//!    `%APPDATA%`. The Windows portable zip ships with `portable.txt`; the MSI doesn't. If that
//!    folder can't be written (read-only medium, Program Files), a warning is logged and the
//!    platform folder is used.
//! 3. The platform convention: macOS `~/Library/Application Support/Unwritten Studio`, Windows
//!    `%APPDATA%\Unwritten Studio`, Linux `$XDG_CONFIG_HOME/unwritten-studio` or `~/.config/unwritten-studio`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Files beside the executable that switch on portable mode (either one; contents are ignored).
pub const PORTABLE_MARKERS: [&str; 2] = ["portable.txt", "Unwritten Studio.portable"];
/// The data folder created beside the executable in portable mode.
pub const PORTABLE_DATA_DIR: &str = "Unwritten StudioData";

/// How [`DataDir::dir`] was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `UNWRITTEN_STUDIO_CONFIG_DIR`.
    Override,
    /// `<exe dir>/Unwritten StudioData`.
    Portable,
    /// The platform's per-user config directory.
    Platform,
}

/// The resolved data directory.
#[derive(Clone, Debug)]
pub struct DataDir {
    /// `None` only when no platform directory is known either (no `HOME`/`APPDATA`).
    pub dir: Option<PathBuf>,
    pub mode: Mode,
    /// Set when a portable marker was found but its data folder isn't writable.
    pub warning: Option<String>,
}

fn process_env(key: &str) -> Option<OsString> {
    std::env::var_os(key)
}

/// The data directory for this process, resolved once (the portable check probes the disk).
pub fn current() -> &'static DataDir {
    static DIR: OnceLock<DataDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok();
        let env = process_env;
        let mut d = resolve(&env, exe.as_deref().and_then(Path::parent));
        if d.mode == Mode::Platform
            && let Some(target) = d.dir.as_deref()
            && let Some(source) = legacy_linux_dir(&env)
            && source != target
            && source.is_dir()
            && !target.exists()
            && let Err(e) = migrate_dir(&source, target)
        {
            let message = format!("could not migrate settings from {} to {}: {e}; the original data was left untouched", source.display(), target.display());
            if d.warning.is_some() {
                log::warn!("{message}");
            } else {
                d.warning = Some(message);
            }
        }
        if let Some(w) = &d.warning {
            log::warn!("{w}");
        }
        d
    })
}

/// Resolve the data directory from the environment (`env`) and the executable's folder.
pub fn resolve(env: impl Fn(&str) -> Option<OsString>, exe_dir: Option<&Path>) -> DataDir {
    if let Some(d) = env("UNWRITTEN_STUDIO_CONFIG_DIR").or_else(|| env("EFFECTCRAFT_CONFIG_DIR")) {
        return DataDir { dir: Some(PathBuf::from(d)), mode: Mode::Override, warning: None };
    }
    let platform = platform_dir(&env);
    let Some((exe_dir, marker)) = exe_dir.and_then(|d| portable_marker(d).map(|m| (d, m))) else {
        return DataDir { dir: platform, mode: Mode::Platform, warning: None };
    };
    let data = exe_dir.join(PORTABLE_DATA_DIR);
    match ensure_writable(&data) {
        Ok(()) => DataDir { dir: Some(data), mode: Mode::Portable, warning: None },
        Err(e) => {
            let fallback = platform.as_ref().map_or_else(|| "nowhere (no user folder found)".to_string(), |p| p.display().to_string());
            let warning =
                format!("portable mode ({} found) but {} isn't writable ({e}); settings are kept in {fallback} instead", marker.display(), data.display());
            DataDir { dir: platform, mode: Mode::Platform, warning: Some(warning) }
        }
    }
}

/// The platform convention, from `env` (so tests can fake `HOME`/`APPDATA`/`XDG_CONFIG_HOME`).
pub fn platform_dir(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let home = || env("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home().map(|h| h.join("Library/Application Support/Unwritten Studio"));
    }
    if cfg!(target_os = "windows") {
        return env("APPDATA").map(|a| PathBuf::from(a).join("Unwritten Studio"));
    }
    env("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home().map(|h| h.join(".config"))).map(|c| c.join("unwritten-studio"))
}

/// The prior Linux settings folder. Its contents are copied on first launch, never moved or deleted.
fn legacy_linux_dir(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    env("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| env("HOME").map(|h| PathBuf::from(h).join(".config"))).map(|c| c.join("effectcraft"))
}

/// Copy a prior config tree to a staged sibling and atomically publish it; the source is retained.
fn migrate_dir(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    if target.exists() || !source.is_dir() || source == target {
        return Ok(());
    }
    let parent = target.parent().ok_or_else(|| Error::new(ErrorKind::InvalidInput, "config target has no parent"))?;
    std::fs::create_dir_all(parent)?;
    static NEXT_STAGE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let stage = parent.join(format!(".unwritten-studio-migration-{}-{}", std::process::id(), NEXT_STAGE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir(&stage)?;
    let result = (|| {
        copy_tree(source, &stage)?;
        if target.exists() {
            return Ok(());
        }
        std::fs::rename(&stage, target)
    })();
    if stage.exists() {
        let _ = std::fs::remove_dir_all(&stage);
    }
    result
}

fn copy_tree(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let dest = target.join(entry.file_name());
        if kind.is_dir() {
            std::fs::create_dir(&dest)?;
            copy_tree(&entry.path(), &dest)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), dest)?;
        } else {
            return Err(Error::new(ErrorKind::InvalidInput, format!("cannot safely copy non-regular config entry {}", entry.path().display())));
        }
    }
    Ok(())
}

/// The first portable marker file in `exe_dir`, if any.
pub fn portable_marker(exe_dir: &Path) -> Option<PathBuf> {
    PORTABLE_MARKERS.iter().map(|m| exe_dir.join(m)).find(|p| p.is_file())
}

/// Create `dir` and prove a file can be written in it (a folder can exist yet be read-only).
fn ensure_writable(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let probe = dir.join(format!(".write-test-{}", std::process::id()));
    std::fs::write(&probe, b"")?;
    // A leftover probe is harmless; don't fail portable mode over it.
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp(tag: &str) -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("unwritten-studio-app-dirs-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn env_of(pairs: &[(&str, &Path)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let m: HashMap<String, OsString> = pairs.iter().map(|(k, v)| (k.to_string(), v.as_os_str().to_owned())).collect();
        move |k| m.get(k).cloned()
    }

    /// A fake user folder that every platform's convention resolves under.
    fn fake_home(root: &Path) -> impl Fn(&str) -> Option<OsString> + use<> {
        let home = root.join("home");
        let appdata = root.join("appdata");
        env_of(&[("HOME", &home), ("APPDATA", &appdata)])
    }

    #[test]
    fn no_marker_uses_the_platform_dir() {
        let root = temp("platform");
        let exe = root.join("bin");
        std::fs::create_dir_all(&exe).unwrap();
        let d = resolve(fake_home(&root), Some(&exe));
        assert_eq!(d.mode, Mode::Platform);
        assert!(d.warning.is_none());
        let dir = d.dir.unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(dir, root.join("home/Library/Application Support/Unwritten Studio"));
        } else if cfg!(target_os = "windows") {
            assert_eq!(dir, root.join("appdata").join("Unwritten Studio"));
        } else {
            assert_eq!(dir, root.join("home/.config/unwritten-studio"));
        }
        assert!(!exe.join(PORTABLE_DATA_DIR).exists(), "no portable folder without a marker");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn either_marker_keeps_everything_beside_the_exe() {
        for marker in PORTABLE_MARKERS {
            let root = temp("portable");
            let exe = root.join("Unwritten Studio");
            std::fs::create_dir_all(&exe).unwrap();
            std::fs::write(exe.join(marker), b"").unwrap();
            let d = resolve(fake_home(&root), Some(&exe));
            assert_eq!(d.mode, Mode::Portable, "{marker}");
            assert!(d.warning.is_none());
            let dir = d.dir.unwrap();
            assert_eq!(dir, exe.join("Unwritten StudioData"));
            assert!(dir.is_dir(), "the data folder is created");
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "the write probe is removed");
            assert!(!root.join("home").exists() && !root.join("appdata").exists(), "nothing written to the user folder");
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn a_marker_folder_is_not_a_marker() {
        let root = temp("dir-marker");
        let exe = root.join("bin");
        std::fs::create_dir_all(exe.join("portable.txt")).unwrap();
        assert_eq!(resolve(fake_home(&root), Some(&exe)).mode, Mode::Platform);
        assert!(!exe.join(PORTABLE_DATA_DIR).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unwritable_portable_folder_falls_back_with_a_warning() {
        let root = temp("unwritable");
        let exe = root.join("bin");
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::write(exe.join("portable.txt"), b"").unwrap();
        // A file where the data folder should go: it can't be created, on every platform.
        std::fs::write(exe.join(PORTABLE_DATA_DIR), b"in the way").unwrap();
        let env = fake_home(&root);
        let d = resolve(&env, Some(&exe));
        assert_eq!(d.mode, Mode::Platform);
        assert_eq!(d.dir, platform_dir(&env));
        assert!(d.dir.is_some());
        let w = d.warning.unwrap();
        assert!(w.contains("portable") && w.contains("isn't writable"), "{w}");
        // No user folder at all: still no panic, just no directory.
        let d = resolve(|_| None, Some(&exe));
        assert!(d.dir.is_none() && d.warning.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn override_wins_over_portable() {
        let root = temp("override");
        let exe = root.join("bin");
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::write(exe.join("portable.txt"), b"").unwrap();
        let custom = root.join("custom");
        let d = resolve(env_of(&[("UNWRITTEN_STUDIO_CONFIG_DIR", &custom)]), Some(&exe));
        assert_eq!((d.mode, d.dir), (Mode::Override, Some(custom)));
        assert!(!exe.join(PORTABLE_DATA_DIR).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn new_override_wins_and_legacy_override_remains_supported() {
        let root = temp("env-alias");
        let old = root.join("old");
        let new = root.join("new");
        let old_only = resolve(env_of(&[("EFFECTCRAFT_CONFIG_DIR", &old)]), None);
        assert_eq!((old_only.mode, old_only.dir), (Mode::Override, Some(old)));
        let both = resolve(env_of(&[("UNWRITTEN_STUDIO_CONFIG_DIR", &new), ("EFFECTCRAFT_CONFIG_DIR", &root)]), None);
        assert_eq!((both.mode, both.dir), (Mode::Override, Some(new)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migration_copies_nested_data_without_removing_the_source() {
        let root = temp("migration");
        let old = root.join("old");
        let new = root.join("new");
        std::fs::create_dir_all(old.join("Plug-ins")).unwrap();
        std::fs::write(old.join("prefs.json"), b"settings").unwrap();
        std::fs::write(old.join("Plug-ins/example.wasm"), b"plugin").unwrap();
        migrate_dir(&old, &new).unwrap();
        assert_eq!(std::fs::read(new.join("prefs.json")).unwrap(), b"settings");
        assert_eq!(std::fs::read(new.join("Plug-ins/example.wasm")).unwrap(), b"plugin");
        assert_eq!(std::fs::read(old.join("prefs.json")).unwrap(), b"settings");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_exe_dir_or_env_never_panics() {
        let d = resolve(|_| None, None);
        assert_eq!((d.mode, d.dir), (Mode::Platform, None));
    }
}
