//! Native AEP export boundary: the exporter returns bytes and never mutates session state.
use serde_json::{Value, json};
use unwritten_studio_aep_export::{self as exporter, ExportOptions};

use crate::commands::{b_p, bad, str_p};
use crate::{EngineError, Result, Session};

fn aep_path(path: &str) -> bool {
    std::path::Path::new(path).extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("aep"))
}

fn export_aep(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").ok_or_else(|| bad("file.exportAep", "missing `path` (.aep)"))?;
    if !aep_path(path) {
        return Err(bad("file.exportAep", "path must end in .aep; native project saves remain .unw"));
    }
    for key in ["allowLosses", "dryRun"] {
        if p.get(key).is_some_and(|v| !v.is_boolean()) {
            return Err(bad("file.exportAep", format!("`{key}` must be a boolean")));
        }
    }
    let allow_losses = b_p(p, "allowLosses").unwrap_or(false);
    let dry_run = b_p(p, "dryRun").unwrap_or(false);
    if dry_run {
        let report = exporter::analyze(&s.project);
        return Ok(json!({"format":"aep", "path":path, "dryRun":true, "written":false, "report":report}));
    }
    if s.path.as_deref().is_some_and(|current| current == path) {
        return Err(EngineError::Other("AEP export is copy-only and cannot overwrite the open project path".into()));
    }
    if s.services.exists(path) {
        return Err(EngineError::Other("AEP export refuses to overwrite an existing file; choose a new destination".into()));
    }
    let result = exporter::export(&s.project, &ExportOptions { allow_losses }).map_err(|e| EngineError::Other(format!("cannot export AEP: {e}")))?;
    let bytes = result.bytes.len();
    s.services.write_file(path, &result.bytes).map_err(|e| EngineError::Other(format!("cannot write {path}: {e}")))?;
    s.toast(format!("Exported AEP copy {path} ({bytes} bytes)"));
    Ok(json!({"format":"aep", "path":path, "dryRun":false, "written":true, "bytes":bytes, "report":result.report, "allowLosses":allow_losses}))
}

pub(crate) fn specs() -> Vec<crate::commands::CommandSpec> {
    vec![crate::cmd!(
        "file.exportAep",
        "Export Project to After Effects AEP...",
        ["File", "Export"],
        None,
        "{path: .aep, dryRun?: bool, allowLosses?: bool}",
        crate::commands::always,
        export_aep
    )]
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use serde_json::json;

    #[test]
    fn refuses_non_aep_and_never_changes_dirty_state() {
        let mut s = Session::default();
        s.execute("file.openDemoProject", json!({})).unwrap();
        let before = s.is_dirty();
        assert!(s.execute("file.exportAep", json!({"path":"copy.json"})).is_err());
        assert_eq!(s.is_dirty(), before);
    }

    #[test]
    fn dry_run_does_not_write_or_change_path() {
        let mut s = Session::default();
        s.execute("file.openDemoProject", json!({})).unwrap();
        let before = s.path.clone();
        let out = s.execute("file.exportAep", json!({"path":"copy.aep", "dryRun":true})).unwrap();
        assert_eq!(out["dryRun"], true);
        assert_eq!(out["written"], false);
        assert_eq!(s.path, before);
    }

    #[test]
    fn exports_binary_copy_without_saving_native_edits() {
        let dir = std::env::temp_dir().join(format!("unwritten-aep-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let native = dir.join("master.unw");
        let output = dir.join("copy.AEP");
        let mut s = Session::default();
        s.execute("comp.new", json!({"name":"Copy test","width":320,"height":180,"duration":2,"frameRate":30})).unwrap();
        s.execute("file.saveAs", json!({"path":native})).unwrap();
        s.execute("layer.newSolid", json!({"name":"Original solid","width":32,"height":32,"color":"#123456"})).unwrap();
        let document = s.project.to_json();
        let path = s.path.clone();
        assert!(s.is_dirty());
        let dry = s.execute("file.exportAep", json!({"path":output,"dryRun":true})).unwrap();
        assert_eq!(dry["report"]["losses"], json!([]));
        assert!(!output.exists());
        let result = s.execute("file.exportAep", json!({"path":output})).unwrap();
        assert_eq!(result["written"], true);
        assert_eq!(s.project.to_json(), document);
        assert_eq!(s.path, path);
        assert!(s.is_dirty());
        let bytes = std::fs::read(&output).unwrap();
        assert_eq!(bytes.get(..4), Some(b"RIFX".as_slice()));
        assert_eq!(bytes.get(8..12), Some(b"Egg!".as_slice()));
        let mut reopened = Session::default();
        reopened.execute("file.open", json!({"path":output})).unwrap();
        assert_eq!(reopened.project.comps().count(), 1);
        assert_eq!(reopened.project.comps().map(|(_, c)| c.layers.len()).sum::<usize>(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn never_overwrites_existing_destination_and_never_silently_allows_losses() {
        let dir = std::env::temp_dir().join(format!("unwritten-aep-refusal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sentinel = dir.join("original.aep");
        let partial = dir.join("partial.aep");
        std::fs::write(&sentinel, b"original must survive").unwrap();
        let mut s = Session::default();
        s.execute("file.openDemoProject", json!({})).unwrap();
        let before = s.project.to_json();
        let dry = s.execute("file.exportAep", json!({"path":sentinel,"dryRun":true})).unwrap();
        assert_eq!(dry["written"], false);
        assert!(s.execute("file.exportAep", json!({"path":sentinel,"allowLosses":true})).is_err());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"original must survive");
        assert!(s.execute("file.exportAep", json!({"path":partial})).is_err());
        assert!(!partial.exists());
        let result = s.execute("file.exportAep", json!({"path":partial,"allowLosses":true})).unwrap();
        assert_eq!(result["written"], true);
        assert!(result["report"]["losses"].as_array().is_some_and(|a| !a.is_empty()));
        assert_eq!(s.project.to_json(), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_write_keeps_document_unchanged() {
        struct Unwritable;
        impl crate::Services for Unwritable {
            fn read_file(&self, _: &str) -> std::io::Result<Vec<u8>> {
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            }
            fn write_file(&self, _: &str, _: &[u8]) -> std::io::Result<()> {
                Err(std::io::Error::other("synthetic full disk"))
            }
            fn exists(&self, _: &str) -> bool {
                false
            }
        }
        let mut s = Session { services: std::sync::Arc::new(Unwritable), ..Session::default() };
        s.execute("comp.new", json!({"width":320,"height":180,"duration":2,"frameRate":30})).unwrap();
        let before = s.project.to_json();
        let dirty = s.is_dirty();
        assert!(s.execute("file.exportAep", json!({"path":"copy.aep"})).is_err());
        assert_eq!(s.project.to_json(), before);
        assert_eq!(s.is_dirty(), dirty);
        assert!(s.path.is_none());
    }

    #[test]
    fn non_boolean_safety_flags_are_refused() {
        let mut s = Session::default();
        assert!(s.execute("file.exportAep", json!({"path":"copy.aep","dryRun":"true"})).is_err());
        assert!(s.execute("file.exportAep", json!({"path":"copy.aep","allowLosses":1})).is_err());
    }
}
