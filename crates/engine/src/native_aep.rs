//! Native, read-only AEP import. Never write imported content over an AEP source.
use unwritten_studio_aep_import::{ImportOptions, ImportReport};
use unwritten_studio_project::Project;

pub(crate) const MAX_AEP_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn is_aep_path(path: &str) -> bool {
    std::path::Path::new(path).extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("aep"))
}

pub(crate) fn check_file_size(path: &str, services: &dyn crate::Services) -> Result<(), String> {
    if is_aep_path(path) && services.stamp(path).is_some_and(|(size, _)| size > MAX_AEP_BYTES as u64) {
        return Err("AEP exceeds the experimental native import limit of 64 MiB".into());
    }
    Ok(())
}

pub(crate) fn decode(bytes: &[u8], path: &str, services: &dyn crate::Services, allow_losses: bool) -> Result<(Project, ImportReport, Vec<String>), String> {
    if bytes.len() > MAX_AEP_BYTES {
        return Err("AEP exceeds the experimental native import limit of 64 MiB".into());
    }
    let folder = std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("After Effects Project");
    let exists = |p: &str| services.exists(p);
    let options = ImportOptions { allow_losses, exists: &exists, folder_name: folder };
    let mut project = Project::default();
    let result = unwritten_studio_aep_import::import(&mut project, bytes, &options).map_err(|e| format!("cannot open AEP: {e}"))?;
    if result.comps.is_empty() {
        return Err("AEP contains no readable compositions; current project has not been replaced".into());
    }
    let mut warnings =
        vec!["Experimental native AEP import. Original AEP is unchanged; save an editable copy as .unw. Not a full-fidelity AE project round trip.".into()];
    if !result.report.losses.is_empty() {
        warnings.push(format!("AEP content not preserved: {}", result.report.summary()));
    }
    append_effect_warnings(&result.report, &mut warnings);
    if !result.report.missing_footage.is_empty() {
        warnings.push(format!("{} linked media files are unavailable; transfer and relink the original media.", result.report.missing_footage.len()));
    }
    warnings.extend(result.report.limitations.iter().cloned());
    // The adapter reconstructs native objects. Apply the same repair/validation boundary as native
    // project opening before any session replacement, including hostile dimensions and timing.
    let text = project.to_json();
    let (project, repairs) = Project::from_json_repaired(&text).map_err(|e| format!("AEP project validation failed: {e}"))?;
    for repair in repairs.iter().take(10) {
        warnings.push(format!("AEP value repaired: {repair}"));
    }
    Ok((project, result.report, warnings))
}

fn append_effect_warnings(report: &ImportReport, warnings: &mut Vec<String>) {
    if let Some(effects) = report.losses.iter().find(|loss| loss.category == unwritten_studio_aep_import::LossCategory::Effects) {
        warnings.push("Effects are not imported, including built-in effects and third-party plugins such as Sapphire/BCC. Plugin dependencies are separate from missing media and do not mean the AEP container is corrupt; desktop plugins cannot run in this app.".into());
        warnings.extend(effects.details.iter().take(20).map(|detail| format!("Effect not imported: {detail}")));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn effect_warnings_name_plugin_dependencies_without_calling_the_project_corrupt() {
        use unwritten_studio_aep_import::{ImportReport, Loss, LossCategory};
        let report = ImportReport {
            losses: vec![Loss { category: LossCategory::Effects, count: 2, details: vec!["effect S_WipeCircle".into(), "effect BCC Video Glitch".into()] }],
            ..Default::default()
        };
        let mut warnings = Vec::new();
        super::append_effect_warnings(&report, &mut warnings);
        assert!(warnings.iter().any(|w| w.contains("S_WipeCircle")));
        assert!(warnings.iter().any(|w| w.contains("BCC Video Glitch")));
        assert!(warnings.iter().any(|w| w.contains("separate from missing media")));
    }

    fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = id.to_vec();
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            bytes.push(0);
        }
        bytes
    }
    fn list(kind: &[u8; 4], children: &[u8]) -> Vec<u8> {
        let mut bytes = kind.to_vec();
        bytes.extend_from_slice(children);
        chunk(b"LIST", &bytes)
    }
    fn synthetic_comp() -> Vec<u8> {
        let mut header = vec![0u8; 204];
        for (at, value) in [(4usize, 1u32), (8, 30), (44, 60), (144, 1), (148, 1), (156, 30 << 16)] {
            header[at..at + 4].copy_from_slice(&value.to_be_bytes());
        }
        header[140..142].copy_from_slice(&320u16.to_be_bytes());
        header[142..144].copy_from_slice(&180u16.to_be_bytes());
        let mut children = chunk(b"idta", &4u16.to_be_bytes());
        children.extend(chunk(b"Utf8", b"Synthetic AE composition"));
        children.extend(chunk(b"cdta", &header));
        let payload = list(b"Fold", &list(b"Item", &children));
        let mut bytes = b"RIFX".to_vec();
        bytes.extend_from_slice(&((payload.len() + 4) as u32).to_be_bytes());
        bytes.extend_from_slice(b"Egg!");
        bytes.extend(payload);
        bytes
    }
    struct Source(Vec<u8>);
    impl crate::Services for Source {
        fn read_file(&self, _: &str) -> std::io::Result<Vec<u8>> {
            Ok(self.0.clone())
        }
        fn write_file(&self, _: &str, _: &[u8]) -> std::io::Result<()> {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "read-only AEP source"))
        }
        fn exists(&self, _: &str) -> bool {
            false
        }
    }
    fn session(bytes: Vec<u8>) -> crate::Session {
        crate::Session { services: Arc::new(Source(bytes)), check_footage_on_open: false, ..crate::Session::default() }
    }
    #[test]
    fn native_aep_open_is_active_dirty_untitled_copy() {
        let mut s = session(synthetic_comp());
        let result = s.execute("file.open", json!({"path":"source.AEP","allowLosses":false})).unwrap();
        assert_eq!(result["format"], "aep");
        assert_eq!(result["report"]["imported"]["compositions"], 1);
        assert!(s.path.is_none() && s.is_dirty());
        let comp = s.active_comp().unwrap();
        assert_eq!((comp.width, comp.height), (320, 180));
        assert!(!result["warnings"].as_array().unwrap().is_empty());
        assert!(s.execute("file.saveAs", json!({"path":"source.AEP"})).is_err());
        assert!(s.path.is_none() && s.is_dirty());
    }
    #[test]
    fn native_aep_malformed_and_empty_never_replace_current_work() {
        for bytes in [b"broken AEP".to_vec(), b"RIFX\0\0\0\x04Egg!".to_vec()] {
            let mut s = session(bytes);
            s.execute("comp.new", json!({"name":"Existing edit","width":32,"height":24})).unwrap();
            let before = s.project.to_json();
            assert!(s.execute("file.open", json!({"path":"source.aep"})).is_err());
            assert_eq!(s.project.to_json(), before);
            assert!(s.path.is_none() && s.is_dirty());
        }
    }
    #[test]
    fn native_aep_import_merges_as_one_undoable_edit() {
        let mut s = session(synthetic_comp());
        s.execute("comp.new", json!({"name":"Existing edit","width":32,"height":24})).unwrap();
        let before = s.project.to_json();
        s.execute("file.importProject", json!({"path":"source.aep"})).unwrap();
        assert_eq!(s.project.comps().count(), 2);
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.project.to_json(), before);
    }
}
