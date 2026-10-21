//! Local recovery for the embedded host. This is deliberately separate from project Save: it never
//! writes the user's source path and restored sessions always remain untitled.
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use unwritten_studio_engine::Session;
use unwritten_studio_project::Project;

const MAX_CHECKPOINT_BYTES: usize = 32 * 1024 * 1024;
const CHECKPOINT_NAME: &str = "embedded-session-recovery.json";
const DEBOUNCE: Duration = Duration::from_secs(2);
const CAP: Duration = Duration::from_secs(5);

pub struct NativeSessionRecovery {
    path: Option<PathBuf>,
    last_revision: u64,
    changed_at: Option<Instant>,
    last_write: Option<Instant>,
    blocked: bool,
}

impl NativeSessionRecovery {
    pub fn new(config_dir: Option<PathBuf>) -> Self {
        Self { path: config_dir.map(|dir| dir.join(CHECKPOINT_NAME)), last_revision: 0, changed_at: None, last_write: None, blocked: false }
    }

    /// Restore once, before the first embedded frame. A malformed checkpoint is retained so it can
    /// be diagnosed, and the caller can surface the returned error without losing the file.
    pub fn restore(&mut self, session: &mut Session) -> Result<bool, String> {
        let result = self.restore_inner(session);
        if result.is_err() {
            self.blocked = true;
        }
        result
    }
    fn restore_inner(&self, session: &mut Session) -> Result<bool, String> {
        let Some(path) = &self.path else { return Ok(false) };
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(format!("could not read local recovery: {error}")),
        };
        let mut text = String::new();
        file.take(MAX_CHECKPOINT_BYTES as u64 + 1).read_to_string(&mut text).map_err(|e| format!("could not read local recovery: {e}"))?;
        if text.len() > MAX_CHECKPOINT_BYTES {
            return Err("local recovery is larger than 32 MiB".into());
        }
        let mut root: Value = serde_json::from_str(&text).map_err(|e| format!("local recovery is corrupt: {e}"))?;
        let app = root.get("app").cloned().unwrap_or_else(|| json!({}));
        let dirty = app.get("dirty").and_then(Value::as_bool).unwrap_or(true);
        let view_text = serde_json::to_string(&root).map_err(|e| format!("local recovery metadata is invalid: {e}"))?;
        let view = unwritten_studio_project::saved_view(&view_text);
        let Some(object) = root.as_object_mut() else { return Err("local recovery is not a JSON object".into()) };
        object.remove("app");
        let project_text = serde_json::to_string(&root).map_err(|e| format!("local recovery project is invalid: {e}"))?;
        let (project, _) = Project::from_json_repaired(&project_text).map_err(|e| format!("local recovery project is invalid: {e}"))?;
        let nonempty = !project.items.is_empty();
        session.replace_project(project, None);
        session.restore_view(&view);
        if dirty || nonempty {
            session.mark_unsaved();
        }
        Ok(nonempty)
    }

    pub fn tick(&mut self, session: &Session, force: bool) -> Result<(), String> {
        if self.blocked {
            return Ok(());
        } // Never replace a corrupt recovery with a blank session.
        let result = self.tick_inner(session, force);
        if result.is_err() {
            let now = Instant::now();
            self.changed_at = Some(now);
            self.last_write = Some(now);
        }
        result
    }
    fn tick_inner(&mut self, session: &Session, force: bool) -> Result<(), String> {
        if self.path.is_none() {
            return Ok(());
        }
        let now = Instant::now();
        if session.revision != self.last_revision {
            self.last_revision = session.revision;
            self.changed_at.get_or_insert(now);
        }
        let due = force
            || self.changed_at.is_some_and(|at| now.duration_since(at) >= DEBOUNCE)
            || self.last_write.is_some_and(|at| now.duration_since(at) >= CAP && self.changed_at.is_some());
        if !due {
            return Ok(());
        }
        let Some(path) = self.path.as_ref() else { return Ok(()) };
        let view = session.saved_view();
        let project_text = session.project.to_file_json_with_view(&view).map_err(|e| format!("local recovery serialization failed: {e}"))?;
        if project_text.len() > MAX_CHECKPOINT_BYTES {
            return Err("local recovery exceeds 32 MiB".into());
        }
        let mut root: Value = serde_json::from_str(&project_text).map_err(|e| format!("local recovery serialization failed: {e}"))?;
        root["app"] = json!({"dirty": session.is_dirty(), "source_path": session.path, "saved_view": view});
        let data = serde_json::to_vec(&root).map_err(|e| format!("local recovery serialization failed: {e}"))?;
        if data.len() > MAX_CHECKPOINT_BYTES {
            return Err("local recovery exceeds 32 MiB".into());
        }
        atomic_write(path, &data).map_err(|e| format!("could not write local recovery: {e}"))?;
        self.last_write = Some(now);
        self.changed_at = None;
        Ok(())
    }
}

fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_file_name(format!("{}.tmp", path.file_name().unwrap_or_default().to_string_lossy()));
    fs::write(&temp, data)?;
    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unwritten_studio_engine::Session;
    #[test]
    fn checkpoint_round_trip_is_untitled_and_dirty() {
        let dir = std::env::temp_dir().join(format!("unwritten-recovery-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut s = Session::default();
        s.execute("comp.new", json!({"name":"Recovered","width":16,"height":16,"duration":1})).expect("project");
        let mut writer = NativeSessionRecovery::new(Some(dir.clone()));
        writer.tick(&s, true).expect("write");
        let mut restored = Session::default();
        let mut reader = NativeSessionRecovery::new(Some(dir.clone()));
        assert!(reader.restore(&mut restored).expect("restore"));
        assert!(restored.path.is_none() && restored.is_dirty() && !restored.project.items.is_empty());
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn oversized_or_corrupt_checkpoint_is_rejected_and_retained() {
        let dir = std::env::temp_dir().join(format!("unwritten-recovery-bad-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(CHECKPOINT_NAME);
        fs::write(&path, vec![b'x'; MAX_CHECKPOINT_BYTES + 1]).expect("fixture");
        let result = NativeSessionRecovery::new(Some(dir.clone())).restore(&mut Session::default());
        assert!(result.is_err() && path.is_file());
        fs::write(&path, b"corrupt").expect("corrupt");
        let mut blocked = NativeSessionRecovery::new(Some(dir.clone()));
        assert!(blocked.restore(&mut Session::default()).is_err());
        blocked.tick(&Session::default(), true).expect("blocked writes are skipped");
        assert_eq!(fs::read(&path).expect("retained"), b"corrupt");
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn failed_checkpoint_can_retry_without_discarding_the_open_project() {
        let dir = std::env::temp_dir().join(format!("unwritten-recovery-retry-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_file(&dir);
        // A non-directory blocks writes portably, including tests run as root.
        fs::write(&dir, b"storage unavailable").expect("block storage");
        let mut session = Session::default();
        session.execute("comp.new", json!({"name":"Keep my work","width":16,"height":16,"duration":1})).expect("project");
        let before = session.project.to_file_json().expect("snapshot");
        let mut recovery = NativeSessionRecovery::new(Some(dir.clone()));
        assert!(recovery.tick(&session, true).is_err());
        assert_eq!(session.project.to_file_json().expect("still open"), before);
        fs::remove_file(&dir).expect("restore storage");
        recovery.tick(&session, true).expect("retry write");
        let mut restored = Session::default();
        assert!(NativeSessionRecovery::new(Some(dir.clone())).restore(&mut restored).expect("restore"));
        assert!(restored.project.to_file_json().expect("restored project").contains("Keep my work"));
        assert!(restored.path.is_none() && restored.is_dirty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn serialization_failure_does_not_replace_previous_checkpoint() {
        let dir = std::env::temp_dir().join(format!("unwritten-recovery-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut s = Session::default();
        s.execute("comp.new", json!({"name":"Stable","width":16,"height":16,"duration":1})).expect("project");
        let mut writer = NativeSessionRecovery::new(Some(dir.clone()));
        writer.tick(&s, true).expect("write");
        let path = dir.join(CHECKPOINT_NAME);
        let old = fs::read(&path).expect("old");
        assert!(old.windows(6).any(|w| w == b"Stable"));
        let temp = path.with_file_name(format!("{}.tmp", CHECKPOINT_NAME));
        fs::create_dir(&temp).expect("block staged write");
        assert!(writer.tick(&s, true).is_err());
        assert_eq!(fs::read(&path).expect("retained"), old);
        let _ = fs::remove_dir_all(dir);
    }
}
