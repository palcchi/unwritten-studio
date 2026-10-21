//! File menu.

use serde_json::{Value, json};
use unwritten_studio_color::Label;
use unwritten_studio_project::{FootageKind, ItemId, ItemKind, LayerSource, Project};

use super::{CommandSpec, always, b_p, bad, str_p};
use crate::sequence::{SequenceOptions, Source};
use crate::{EngineError, Result, Session, cmd};

fn has_path(s: &Session) -> std::result::Result<(), String> {
    if s.path.is_some() { Ok(()) } else { Err("the project has not been saved yet".into()) }
}

fn new_project(s: &mut Session, _: &Value) -> Result<Value> {
    // Settings ▸ Project ▸ New Project Loads Template: open the template as an untitled project.
    let tpl = s.prefs.project.template_path.trim().to_string();
    if s.prefs.project.use_template && !tpl.is_empty() {
        let bytes = s.services.read_file(&tpl).map_err(|e| EngineError::Other(format!("cannot read the new project template {tpl}: {e}")))?;
        let text = String::from_utf8(bytes).map_err(|_| EngineError::Other("the new project template is not a project file".into()))?;
        s.replace_project(Project::from_json(&text)?, None);
        s.update_sentinel();
        return Ok(json!({"template": tpl}));
    }
    s.replace_project(Project::default(), None);
    s.update_sentinel();
    Ok(Value::Null)
}

fn demo(s: &mut Session, _: &Value) -> Result<Value> {
    let p = crate::demo::demo_project();
    s.replace_project(p, None);
    if let Some(id) = s.project.items.values().find(|i| i.name == crate::demo::MAIN_COMP).map(|i| i.id) {
        s.open_comp(id);
        s.set_time(unwritten_studio_time::Tick::from_seconds_f64(2.5));
    }
    Ok(json!({"comp": s.state.active_comp.map(|c| c.0)}))
}

pub(crate) fn open(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").ok_or_else(|| bad("file.open", "missing `path`"))?;
    reject_after_effects(path, "open")?;
    let bytes = s.services.read_file(path).map_err(|e| EngineError::Other(format!("cannot read {path}: {e}")))?;
    if path.to_ascii_lowercase().ends_with(".unwae") {
        let (mut project, warnings) = crate::ae_bridge::import_unwae(&bytes).map_err(EngineError::Other)?;
        resolve_relative_footage(&mut project, path, s.services.as_ref());
        // Interchange is not a native project; never let Save overwrite it.
        s.replace_project(project, None);
        // AE's project-bin order can put a nested precomp before the final composition.
        // Open a root, not an arbitrary first item, so the imported edit is visible.
        if let Some(root) = super::import_project::root_comps(&s.project).first().copied() {
            s.open_comp(root);
        }
        for warning in warnings.iter().take(10) {
            s.toast(warning.clone());
        }
        if s.check_footage_on_open {
            crate::footage_check::start(s, None, false)?;
        }
        return Ok(json!({"path": path, "format": "unwae", "warnings": warnings}));
    }
    let text = String::from_utf8(bytes).map_err(|_| EngineError::Other("not a text project file".into()))?;
    // XML copies (File ▸ Save a Copy As XML…) open like the JSON project.
    let text = if crate::xml_project::is_xml(&text) { crate::xml_project::from_xml(&text).map_err(EngineError::Other)? } else { text };
    let (mut proj, repaired) =
        Project::from_json_repaired(&text).map_err(|e| EngineError::Other(format!("{path} is not a project Unwritten Studio can open: {e}")))?;
    resolve_relative_footage(&mut proj, path, s.services.as_ref());
    s.replace_project(proj, Some(path.to_string()));
    s.restore_view(&unwritten_studio_project::saved_view(&text));
    s.note_project_path(path);
    // Values past the limits (a corrupt or hostile file) were brought within them (#788).
    if !repaired.is_empty() {
        let more = repaired.len().saturating_sub(3);
        let shown = repaired.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
        let more = if more > 0 { format!(" and {more} more") } else { String::new() };
        s.toast(format!("This project had values outside Unwritten Studio's limits, corrected when it opened: {shown}{more}."));
    }
    // Written by a newer Unwritten Studio: what this version doesn't know would be lost on saving.
    let saved_by = unwritten_studio_project::saved_by(&text);
    if let Some(v) = saved_by.as_deref().filter(|v| unwritten_studio_project::is_newer_version(v, unwritten_studio_project::APP_VERSION)) {
        s.toast(format!(
            "This project was saved by Unwritten Studio {v}, newer than this version ({}). Settings this version doesn't know are lost if you save it.",
            unwritten_studio_project::APP_VERSION
        ));
    }
    // Lazy open: footage is checked in the background (Progress panel), not before the
    // project shows.
    if s.check_footage_on_open {
        crate::footage_check::start(s, None, false)?;
    }
    Ok(json!({"path": path, "savedBy": saved_by, "repaired": repaired}))
}

/// Footage saved with a relative path (imported by the CLI or an agent as `clip.mp4`, or written
/// by hand) is found next to the project file, as After Effects looks for footage relative to
/// the project. It used to be looked up from the working directory: the CLI started in the
/// project's folder found it, the desktop app (started from the Finder, Explorer or a launcher,
/// in another folder) showed and rendered nothing (#652). A path that exists from the working
/// directory is kept.
fn resolve_relative_footage(p: &mut Project, project_path: &str, services: &dyn crate::Services) {
    use std::path::Path;
    let Some(dir) = Path::new(project_path).parent().filter(|d| !d.as_os_str().is_empty()) else { return };
    let beside = |f: &str| dir.join(f).to_string_lossy().to_string();
    for item in p.items.values_mut() {
        let ItemKind::Footage(f) = &mut item.kind else { continue };
        if f.path.is_empty() || !Path::new(&f.path).is_relative() || services.exists(&f.path) || !services.exists(&beside(&f.path)) {
            continue;
        }
        f.path = beside(&f.path);
        for frame in f.sequence.iter_mut().filter(|q| Path::new(q.as_str()).is_relative()) {
            *frame = beside(frame);
        }
    }
}

/// The project file's text for `path`: `.unw` (or legacy `.uwproj`) JSON, or XML for `.uwprojx` / `.xml` (a
/// project opened from an XML copy saves as XML again). Fails, writing nothing, when the project
/// wouldn't read back.
pub(crate) fn file_text(s: &Session, path: &str) -> Result<String> {
    reject_after_effects(path, "save")?;
    if path.to_ascii_lowercase().ends_with(".unwae") {
        return Err(EngineError::Other("Save native projects as .unw; .unwae is an import-only AE interchange, not a native project.".into()));
    }
    let json = s.project.to_file_json_with_view(&s.saved_view())?;
    if crate::xml_project::is_xml_path(path) { crate::xml_project::to_xml(&json).map_err(EngineError::Other) } else { Ok(json) }
}

/// AEP/AEPX are proprietary formats; this clean-room project has no codec for them.
pub(super) fn reject_after_effects(path: &str, operation: &str) -> Result<()> {
    let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("aep") || ext.eq_ignore_ascii_case("aepx") {
        return Err(EngineError::Other(format!(
            "cannot {operation} `{path}`: direct After Effects .aep/.aepx is not supported. Use .unw (native), or File > After Effects Bridge: run its exporter in AE desktop, open the resulting .unwae here; export JSX back to AE to reconstruct supported layers and save a new .aep"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod format_tests {
    use super::reject_after_effects;
    use crate::Session;
    use serde_json::json;

    #[test]
    fn ae_project_import_reports_conversion_without_touching_current_work() {
        let mut session = Session::default();
        session.execute("comp.new", json!({"name":"Keep this edit","width":32,"height":24,"duration":1})).unwrap();
        let before = session.project.to_json();
        for path in ["missing.AEP", "missing.AePx"] {
            let error = session.execute("file.importProject", json!({"path":path})).unwrap_err().to_string();
            assert!(error.contains("After Effects Bridge") && error.contains(".unwae"), "{error}");
            let result = session.execute("file.import", json!({"paths":[path]})).unwrap();
            let errors = result["errors"].as_array().unwrap();
            assert_eq!(errors.len(), 1, "{result}");
            assert!(errors[0].as_str().unwrap().contains("After Effects Bridge"), "{result}");
            assert_eq!(session.project.to_json(), before);
            assert!(session.path.is_none());
        }
    }

    #[test]
    fn ae_bridge_open_activates_root_composition_instead_of_first_precomp() {
        let dir = std::env::temp_dir().join(format!("unwritten-bridge-root-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("converted.unwae");
        let document = json!({"format":"unwritten-after-effects","version":1,"comps":[
            {"id":"nested","name":"Nested first","width":32,"height":24,"fps":24,"duration":1},
            {"id":"root","name":"Final composition","width":32,"height":24,"fps":24,"duration":1,
             "layers":[{"name":"Precomp","kind":"comp","source":"nested","width":32,"height":24,"outPoint":1}]}
        ]});
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let mut session = Session::default();
        session.execute("file.open", json!({"path":path.to_string_lossy()})).unwrap();
        let active = session.state.active_comp.unwrap();
        assert_eq!(session.project.item(active).unwrap().name, "Final composition");
        assert!(session.path.is_none(), "converted interchange must remain an untitled native project");
        assert_eq!(std::fs::read(&path).unwrap(), serde_json::to_vec(&document).unwrap());
        let mut destination = Session::default();
        let result = destination.execute("file.importProject", json!({"path":path.to_string_lossy()})).unwrap();
        assert!(result["warnings"].as_array().unwrap().iter().any(|w| w.as_str().is_some_and(|w| w.contains("Experimental AE bridge"))), "{result}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn native_unw_round_trips() {
        let dir = std::env::temp_dir().join(format!("unwritten-unw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("roundtrip.unw");
        let mut a = Session::default();
        a.execute("comp.new", json!({"name":"Canonical", "width":32, "height":24, "duration":1})).unwrap();
        a.execute("file.saveAs", json!({"path": path.to_string_lossy()})).unwrap();
        let mut b = Session::default();
        b.execute("file.open", json!({"path": path.to_string_lossy()})).unwrap();
        assert_eq!(b.path.as_deref(), Some(path.to_string_lossy().as_ref()));
        assert!(b.project.items.values().any(|i| i.name == "Canonical" && matches!(i.kind, unwritten_studio_project::ItemKind::Comp(_))));
        assert!(super::super::import_project::is_project_path("ROUNDTRIP.UNW"));
        let mut imported = Session::default();
        let result = imported.execute("file.import", json!({"paths": [path.to_string_lossy()]})).unwrap();
        assert!(result["errors"].as_array().is_none_or(Vec::is_empty), "{result}");
        assert!(imported.project.items.values().any(|i| i.name == "Canonical" && matches!(i.kind, unwritten_studio_project::ItemKind::Comp(_))));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_ae_bridge_keeps_document_and_native_save_cannot_overwrite_interchange() {
        let dir = std::env::temp_dir().join(format!("unwritten-bridge-transaction-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("source.unwae");
        let original = br#"{"format":"unwritten-after-effects","version":99,"comps":[]}"#;
        std::fs::write(&path, original).unwrap();
        let mut session = Session::default();
        session.execute("comp.new", json!({"name":"Keep my work","width":32,"height":24,"duration":1})).unwrap();
        let before = session.project.to_json();
        assert!(session.execute("file.open", json!({"path":path.to_string_lossy()})).is_err());
        assert_eq!(session.project.to_json(), before);
        assert!(session.execute("file.saveAs", json!({"path":path.to_string_lossy()})).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(session.path.is_none());
        assert!(super::super::import_project::is_project_path("SOURCE.UNWAE"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ae_script_exports_reject_native_aep_destinations_without_writing() {
        let dir = std::env::temp_dir().join(format!("unwritten-ae-export-guard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Original.AEP");
        let original = b"RIFX\0\0\0\x04Egg!";
        std::fs::write(&path, original).unwrap();
        let mut session = Session::default();
        session.execute("comp.new", json!({"name":"Keep this edit","width":32,"height":24,"duration":1})).unwrap();
        let before = session.project.to_json();
        for command in ["file.exportAfterEffects", "file.exportAfterEffectsExporter"] {
            let error = session.execute(command, json!({"path":path.to_string_lossy()})).unwrap_err().to_string();
            assert!(error.contains(".jsx"), "{error}");
            assert_eq!(std::fs::read(&path).unwrap(), original);
            assert_eq!(session.project.to_json(), before);
            assert!(session.path.is_none());
            let missing = dir.join("NeverCreate.aepx");
            assert!(session.execute(command, json!({"path":missing.to_string_lossy()})).is_err());
            assert!(!missing.exists());
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn proprietary_after_effects_paths_are_rejected_before_io() {
        assert!(reject_after_effects("project.aep", "save").is_err());
        assert!(reject_after_effects("project.aepx", "open").is_err());
        assert!(reject_after_effects("project.UNW", "save").is_ok());
        assert!(reject_after_effects("project.uwproj", "save").is_ok());
        let dir = std::env::temp_dir().join(format!("unwritten-aep-guard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Existing.AEP");
        let original = b"RIFX\0\0\0\x04Egg!";
        std::fs::write(&path, original).unwrap();
        let mut session = Session::default();
        let error = session.execute("file.saveAs", json!({"path": path.to_string_lossy()})).unwrap_err().to_string();
        assert!(error.contains("not supported") && error.contains(".unw"), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let missing = dir.join("Unsupported.aepx");
        assert!(session.execute("file.saveAs", json!({"path": missing.to_string_lossy()})).is_err());
        assert!(!missing.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn save_to(s: &mut Session, path: &str) -> Result<Value> {
    let json = file_text(s, path)?;
    s.services.write_file(path, json.as_bytes()).map_err(|e| EngineError::Other(format!("cannot write {path}: {e}")))?;
    s.path = Some(path.to_string());
    s.mark_saved();
    s.note_project_path(path);
    s.toast(format!("Saved {path}"));
    Ok(json!({"path": path, "bytes": json.len()}))
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").map(str::to_string).or_else(|| s.path.clone()).ok_or_else(|| bad("file.save", "no path: use file.saveAs {path}"))?;
    save_to(s, &path)
}

fn save_as(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").ok_or_else(|| bad("file.saveAs", "missing `path`"))?;
    save_to(s, path)
}

fn increment_save(s: &mut Session, _: &Value) -> Result<Value> {
    let cur = s.path.clone().ok_or_else(|| bad("file.incrementAndSave", "save the project first"))?;
    // After Effects: `Intro.aep` → `Intro 2.aep` → `Intro 3.aep`.
    let next = crate::autosave::increment_path(&cur, |p| s.file_ops().is_file(std::path::Path::new(p)));
    save_to(s, &next)
}

fn revert(s: &mut Session, _: &Value) -> Result<Value> {
    let path = s.path.clone().ok_or_else(|| bad("file.revert", "not saved"))?;
    open(s, &json!({"path": path}))
}

/// File ▸ Import; with `addToComp` (files dropped on the Timeline or the Composition viewer) they
/// also become layers of the active comp ([`add_to_comp`]).
///
/// With `background` (files dropped on the window or picked in the Import dialog) the files are
/// probed on a background thread, so decoding a slow video doesn't freeze the app: the job shows
/// the file being read and how many are left (Window ▸ Progress, `jobs.list`), then adds them in
/// one undo step, selects them and reports how many arrived ("Imported 3 items").
fn import_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let target = s.active_comp_id();
    let mut pending = prepare_import(s, p)?;
    if b_p(p, "background").unwrap_or(false) && !pending.sources.is_empty() {
        let sources = std::mem::take(&mut pending.sources);
        let total = sources.len();
        let label = match sources.first() {
            Some(one) if total == 1 => format!("Importing {}", one.name()),
            _ => format!("Importing {total} files"),
        };
        let p = p.clone();
        return s.spawn_task("import", label, false, move |ctl| {
            let mut probed = Vec::with_capacity(total);
            for (k, src) in sources.iter().enumerate() {
                ctl.message(format!("{} ({} of {total})", src.name(), k + 1));
                if !ctl.progress(k as u64, total as u64) {
                    return Err(crate::render_queue::CANCELLED.into());
                }
                probed.push(pending.prober.probe(src));
            }
            ctl.progress(total as u64, total as u64);
            let apply: crate::jobs::Apply = Box::new(move |s: &mut Session| {
                let made = pending.comps.len() + probed.iter().filter(|r| r.is_ok()).count();
                let mut r = add_footage(s, pending, probed)?;
                finish_import(s, &mut r, target, &p);
                let mut toast = format!("Imported {made} item{}", if made == 1 { "" } else { "s" });
                let errors: Vec<&str> = r["errors"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                match errors.as_slice() {
                    [] => {}
                    [one] => toast = format!("{toast}. {one}"),
                    [first, rest @ ..] => toast = format!("{toast}. {first} (and {} more)", rest.len()),
                }
                r["toast"] = json!(toast);
                Ok(r)
            });
            Ok(apply)
        });
    }
    let probed = pending.sources.iter().map(|src| pending.prober.probe(src)).collect();
    let mut r = add_footage(s, pending, probed)?;
    finish_import(s, &mut r, target, p);
    Ok(r)
}

/// After an import (its result `r`): with `addToComp`, the new items become layers of `target`;
/// what couldn't be added joins the result's errors.
fn finish_import(s: &mut Session, r: &mut Value, target: Option<ItemId>, p: &Value) {
    let errors = add_to_comp(s, r, target, p);
    if !errors.is_empty()
        && let Some(e) = r.get_mut("errors").and_then(Value::as_array_mut)
    {
        e.extend(errors.into_iter().map(Value::from));
    }
}

/// With `addToComp`, put what an import (its result `r`) made into `target`, the comp that was
/// active before it, one layer under the other, at the drop's `time`, `index` and `position`
/// (as in `layer.addItem`): the comps of layered files (their layers' footage stays in the
/// Project panel) and the other footage. `target` stays open. Returns what couldn't be added.
pub(crate) fn add_to_comp(s: &mut Session, r: &Value, target: Option<ItemId>, p: &Value) -> Vec<String> {
    let Some(cid) = target.filter(|_| p.get("addToComp").and_then(Value::as_bool).unwrap_or(false)) else { return vec![] };
    let ids = |k: &str| r.get(k).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).collect::<Vec<u64>>();
    let comps = ids("comps");
    let in_comps: std::collections::HashSet<u64> = comps
        .iter()
        .filter_map(|c| s.project.comp(ItemId(*c)))
        .flat_map(|c| c.layers.iter())
        .filter_map(|l| match l.source {
            LayerSource::Footage { item } | LayerSource::Comp { item } | LayerSource::Solid { item } => Some(item.0),
            _ => None,
        })
        .collect();
    let add: Vec<u64> = comps.iter().copied().chain(ids("items").into_iter().filter(|i| !in_comps.contains(i))).collect();
    let mut errors = vec![];
    for (k, item) in add.into_iter().enumerate() {
        let mut params = json!({"comp": cid.0, "item": item, "time": p.get("time"), "position": p.get("position")});
        if let Some(i) = p.get("index").and_then(Value::as_u64) {
            params["index"] = json!(i.saturating_add(k as u64));
        }
        if let Err(e) = s.execute("layer.addItem", params) {
            errors.push(e.to_string());
        }
    }
    if s.active_comp_id() != Some(cid) {
        s.open_comp(cid);
    }
    errors
}

/// File ▸ Import, all in one go (scripts and commands built on it); see [`import_cmd`].
pub(crate) fn import(s: &mut Session, p: &Value) -> Result<Value> {
    let pending = prepare_import(s, p)?;
    let probed = pending.sources.iter().map(|src| pending.prober.probe(src)).collect();
    add_footage(s, pending, probed)
}

/// An import whose layered files are already compositions: the files and image sequences left
/// to probe as footage, how, and what the import made so far.
struct PendingImport {
    sources: Vec<Source>,
    prober: Prober,
    comps: Vec<u64>,
    items: Vec<u64>,
    errors: Vec<String>,
    /// Unwritten Studio projects merged in (`file.importProject` results); never added to a comp.
    projects: Vec<Value>,
}

/// Everything probing a file needs, apart from the session (it can run on a background thread).
struct Prober {
    importer: Option<std::sync::Arc<dyn crate::Importer>>,
    services: std::sync::Arc<dyn crate::Services>,
    /// Settings ▸ Import ▸ Report Missing Frames.
    report_missing_frames: bool,
    /// Settings ▸ Import ▸ Interpret Unlabeled Alpha As, for stills and for movies.
    still_alpha: Option<unwritten_studio_project::AlphaMode>,
    movie_alpha: Option<unwritten_studio_project::AlphaMode>,
    /// Image sequences' frames per second: the import's `frameRate`, or Settings ▸ Import ▸
    /// Sequence Footage.
    sequence_rate: unwritten_studio_time::FrameRate,
    /// PDF / Illustrator page (0-based).
    page: u32,
    /// Choose Layer: one layer of a Photoshop document.
    psd_layer: Option<Value>,
}

/// A file (or image sequence) probed as footage.
struct Probed {
    /// The Project panel name.
    name: String,
    footage: unwritten_studio_project::Footage,
    /// Its alpha is unlabeled and Settings say Ask User: open Interpret Footage.
    ask_alpha: bool,
    warning: Option<String>,
}

impl Prober {
    /// Probe a file or image sequence as footage; the error names the file.
    fn probe(&self, src: &Source) -> std::result::Result<Probed, String> {
        let path = src.path();
        let name = src.name();
        let fail = |e: String| format!("{path}: {e}");
        // Data files (JSON, CSV, TSV) for data-driven animation: kept as text in the project.
        if let Source::File(_) = src
            && let Some(f) = data_footage(self.services.as_ref(), path)
        {
            return f.map(|footage| Probed { name, footage, ask_alpha: false, warning: None }).map_err(fail);
        }
        let importer = self.importer.as_ref().ok_or_else(|| fail("media import is not available in this build".into()))?;
        let mut f = crate::sequence::probe(importer.as_ref(), src, self.sequence_rate).map_err(fail)?;
        let mut warning = None;
        // Settings ▸ Import ▸ Report Missing Frames (numbered sequences).
        if f.kind == FootageKind::Sequence
            && !f.alphabetical
            && self.report_missing_frames
            && let Some(gaps) = crate::sequence::missing_report(&f.sequence)
        {
            warning = Some(format!("{name}: {gaps} (shown as placeholders)"));
        }
        // Settings ▸ Import ▸ Interpret Unlabeled Alpha As: images only (a 3D model renders its
        // own alpha, and audio and data have none).
        let mut ask_alpha = false;
        if matches!(f.kind, FootageKind::Still | FootageKind::Sequence | FootageKind::Video)
            && f.alpha != unwritten_studio_project::AlphaMode::Ignore
            && !alpha_is_labeled(path, &f.codec)
        {
            let setting = if f.kind == FootageKind::Video { self.movie_alpha } else { self.still_alpha };
            match setting {
                Some(a) => f.alpha = a,
                None => ask_alpha = true,
            }
        }
        // Page: another page of a PDF / Illustrator file.
        if self.page > 0 && matches!(f.codec.as_str(), "PDF" | "AI") {
            let doc = self
                .services
                .read_file(path)
                .map_err(|e| e.to_string())
                .and_then(|b| unwritten_studio_pdf::parse_page(&b, self.page as usize).map_err(|e| e.to_string()))
                .map_err(fail)?;
            (f.width, f.height) = doc.pixel_size();
            f.page = self.page;
        }
        // Choose Layer: one layer of a Photoshop document (document-sized).
        if let Some(sel) = &self.psd_layer
            && f.codec == "PSD"
        {
            let (index, name) = self
                .services
                .read_file(path)
                .ok()
                .and_then(|b| unwritten_studio_psd::Psd::parse(b).ok())
                .and_then(|d| find_psd_layer(&d, sel))
                .ok_or_else(|| fail(format!("no layer {sel}")))?;
            f.layer = Some(unwritten_studio_project::SourceLayer { index: index as u32, name, layer_size: false, embedded: None, placed: false });
        }
        Ok(Probed { name, footage: f, ask_alpha, warning })
    }
}

/// The first part of an import: read the parameters, note the files as recent footage, and
/// import layered files (Photoshop, PDF, Illustrator, EPS) as compositions when `importAs` (or,
/// for dropped files, Default Drag Import As) asks for that. The other files are left to probe.
fn prepare_import(s: &mut Session, p: &Value) -> Result<PendingImport> {
    let paths: Vec<String> = match p.get("paths").or(p.get("path")) {
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        Some(Value::String(x)) => vec![x.clone()],
        _ => return Err(bad("file.import", "missing `paths`")),
    };
    // Photoshop documents as compositions (Import As: Composition / – Retain Layer Sizes).
    // Files dragged in follow Settings ▸ Import ▸ Default Drag Import As.
    let drag = p.get("drag").and_then(Value::as_bool).unwrap_or(false);
    let import_as = str_p(p, "importAs").unwrap_or(if drag { s.prefs.drag_import_as() } else { "footage" });
    for path in &paths {
        s.prefs.push_recent_footage(path);
    }
    s.save_prefs();
    let retain = match import_as {
        "footage" => None,
        "composition" | "comp" => Some(false),
        "compositionLayerSizes" | "compositionRetainLayerSizes" | "layerSizes" => Some(true),
        other => return Err(bad("file.import", format!("importAs: footage|composition|compositionLayerSizes, not `{other}`"))),
    };
    // PDF / Illustrator: which page (1-based, as in the Import dialog).
    let page = match p.get("page") {
        None | Some(Value::Null) => 0,
        Some(v) => match v.as_u64().or_else(|| v.as_f64().filter(|x| x.fract() == 0.0 && *x >= 1.0).map(|x| x as u64)) {
            Some(n) if n >= 1 => (n - 1) as u32,
            _ => return Err(bad("file.import", "page: a page number from 1")),
        },
    };
    let mut comps = vec![];
    let mut items = vec![];
    let mut errors = vec![];
    // Unwritten Studio projects import as projects, as After Effects imports an .aep.
    let (project_paths, mut paths): (Vec<String>, Vec<String>) = paths.into_iter().partition(|x| super::import_project::is_project_path(x));
    let mut projects = vec![];
    // AEP is a project container, never footage. Return the actionable bridge
    // instructions even in builds without a media importer, without probing it.
    paths.retain(|path| match reject_after_effects(path, "import") {
        Ok(()) => true,
        Err(error) => {
            errors.push(error.to_string());
            false
        }
    });
    for path in project_paths {
        match s.execute("file.importProject", json!({"path": path})) {
            Ok(r) => projects.push(r),
            Err(e) => errors.push(format!("{path}: {e}")),
        }
    }
    if let Some(retain) = retain {
        let mut rest = vec![];
        for path in paths {
            let bytes = match s.services.read_file(&path) {
                Ok(b) if unwritten_studio_psd::is_psd(&b) => b,
                Ok(b) if unwritten_studio_pdf::sniff(&b).is_some() && !path.to_ascii_lowercase().ends_with(".svg") => {
                    // PDF / Illustrator / EPS: one layer per file layer.
                    match import_vector_comp(s, &path, &b, page) {
                        Ok((comp, made)) => {
                            comps.push(comp);
                            items.extend(made);
                        }
                        Err(e) => errors.push(e.to_string()),
                    }
                    continue;
                }
                _ => {
                    rest.push(path);
                    continue;
                }
            };
            match import_psd_comp(s, &path, bytes, retain) {
                Ok((comp, made, warnings)) => {
                    comps.push(comp);
                    items.extend(made);
                    errors.extend(warnings);
                }
                Err(e) => errors.push(format!("{path}: {e}")),
            }
        }
        paths = rest;
    }
    // Numbered stills as image sequences (the Import dialog's "<format> Sequence" and Force
    // Alphabetical Order), at `frameRate` or Settings ▸ Import ▸ Sequence Footage.
    let opts = SequenceOptions::for_import(p);
    let fps = match p.get("frameRate") {
        None | Some(Value::Null) => s.prefs.import.sequence_fps,
        Some(v) => v.as_f64().filter(|x| *x > 0.0 && *x <= 999.0).ok_or_else(|| bad("file.import", "frameRate: frames per second, up to 999"))?,
    };
    let sources = crate::sequence::group(s.services.as_ref(), &paths, opts);
    let prober = Prober {
        importer: s.importer.clone(),
        services: s.services.clone(),
        report_missing_frames: s.prefs.import.report_missing_frames,
        still_alpha: s.prefs.unlabeled_alpha(false),
        movie_alpha: s.prefs.unlabeled_alpha(true),
        sequence_rate: unwritten_studio_time::FrameRate::from_f64(fps),
        page,
        psd_layer: p.get("layer").cloned(),
    };
    Ok(PendingImport { sources, prober, comps, items, errors, projects })
}

/// The last part of an import: add the probed files to the project (one undo step) and select
/// everything the import made, so the Project panel shows it.
fn add_footage(s: &mut Session, pending: PendingImport, probed: Vec<std::result::Result<Probed, String>>) -> Result<Value> {
    let PendingImport { comps, mut items, mut errors, projects, .. } = pending;
    let mut found = vec![];
    let mut warnings = vec![];
    for r in probed {
        match r {
            Ok(f) => {
                warnings.extend(f.warning.clone());
                found.push(f);
            }
            Err(e) => errors.push(e),
        }
    }
    let mut asked: Vec<u64> = vec![];
    let made: Vec<ItemId> = comps.iter().map(|c| ItemId(*c)).collect();
    if !found.is_empty() {
        // The file may have been overwritten since an earlier import or project decoded it (the
        // media layer keys frames by path): a fresh import reads it again (#1070).
        for f in &found {
            s.forget_decoded(&f.footage);
        }
        s.edit("Import", None, |proj, st| {
            let mut selection = made;
            for f in found {
                let name = f.name;
                let footage = f.footage;
                let name = match &footage.layer {
                    Some(l) => format!("{}/{name}", l.name),
                    None if footage.page > 0 => format!("{name} (Page {})", footage.page + 1),
                    None => name,
                };
                let label = match footage.kind {
                    FootageKind::Still | FootageKind::Sequence => Label::Lavender,
                    FootageKind::Audio => Label::SeaFoam,
                    FootageKind::Video | FootageKind::Model => Label::Aqua,
                    FootageKind::Data => Label::Sandstone,
                };
                let id = proj.add_item(&name, label, None, ItemKind::Footage(footage));
                items.push(id.0);
                if f.ask_alpha {
                    asked.push(id.0);
                }
                selection.push(id);
            }
            st.project_selection = selection;
            Ok(())
        })?;
    }
    if let Some(c) = comps.first() {
        s.open_comp(ItemId(*c));
    }
    for w in &warnings {
        s.events.push(crate::Event::Toast { message: w.clone(), error: true });
    }
    // Ask User: the frontend opens Interpret Footage for footage with unlabeled alpha.
    if let Some(first) = asked.first() {
        s.events.push(crate::Event::Frontend { command: "file.interpretFootage".into(), params: json!({"item": first, "reason": "unlabeledAlpha"}) });
    }
    let mut out = json!({"items": items, "comps": comps, "errors": errors, "warnings": warnings, "unlabeledAlpha": asked});
    if !projects.is_empty() {
        out["projects"] = Value::Array(projects);
    }
    Ok(out)
}

/// Whether a file format states how its alpha is stored (PNG, GIF, WebP and PSD are straight by
/// definition, OpenEXR premultiplied); TGA, TIFF and movies with alpha don't.
fn alpha_is_labeled(path: &str, codec: &str) -> bool {
    let ext = std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    matches!(ext.as_str(), "png" | "gif" | "webp" | "psd" | "psb" | "exr" | "svg" | "jpg" | "jpeg" | "ai" | "pdf" | "eps") || codec.eq_ignore_ascii_case("PSD")
}

/// A Photoshop layer by index or name (pixel layers only).
fn find_psd_layer(d: &unwritten_studio_psd::Psd, sel: &Value) -> Option<(usize, String)> {
    let l = match sel {
        Value::Number(n) => d.layers.get(n.as_u64()? as usize)?,
        Value::String(name) => d.layers.iter().find(|l| &l.name == name)?,
        _ => return None,
    };
    Some((l.index, l.name.clone()))
}

/// Import a Photoshop document as a composition (one undo step). Returns (comp, items, warnings).
pub(crate) fn import_psd_comp(s: &mut Session, path: &str, bytes: Vec<u8>, retain: bool) -> Result<(u64, Vec<u64>, Vec<String>)> {
    let psd = unwritten_studio_psd::Psd::parse(bytes).map_err(|e| EngineError::Other(e.to_string()))?;
    let name = std::path::Path::new(path).file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Photoshop".into());
    let rate = unwritten_studio_time::FrameRate::FPS_29_97;
    let secs = if s.prefs.import.still_footage == "seconds" { s.prefs.import.still_seconds } else { 10.0 };
    let duration = rate.snap_nearest(unwritten_studio_time::Tick::from_seconds_f64(secs));
    let r = s.edit("Import", None, |proj, st| {
        let r = crate::psd_import::import(proj, &psd, path, &name, retain, rate, duration);
        st.project_selection = vec![r.comp];
        Ok(r)
    })?;
    let mut items = vec![r.folder.0];
    items.extend(r.items.iter().map(|i| i.0));
    Ok((r.comp.0, items, r.warnings))
}

/// Import page `page` of a PDF / Illustrator / EPS file as a composition (one undo step).
/// Returns (comp, items).
fn import_vector_comp(s: &mut Session, path: &str, bytes: &[u8], page: u32) -> Result<(u64, Vec<u64>)> {
    let mut name = std::path::Path::new(path).file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Vector".into());
    if page > 0 {
        name = format!("{name} (Page {})", page + 1);
    }
    let rate = unwritten_studio_time::FrameRate::FPS_29_97;
    let secs = if s.prefs.import.still_footage == "seconds" { s.prefs.import.still_seconds } else { 10.0 };
    let duration = rate.snap_nearest(unwritten_studio_time::Tick::from_seconds_f64(secs));
    let (comp, folder, items) = s.edit("Import", None, |proj, st| {
        let r = crate::vector::import_vector_comp(proj, path, bytes, &name, page, rate, duration).map_err(EngineError::Other)?;
        st.project_selection = vec![r.0];
        Ok(r)
    })?;
    let mut out = vec![folder.0];
    out.extend(items.iter().map(|i| i.0));
    Ok((comp.0, out))
}

/// Extensions imported as data footage.
pub const DATA_EXTENSIONS: &[&str] = &["json", "csv", "tsv"];

/// A data footage item for `path` (JSON / CSV / TSV), `None` for other files.
pub(crate) fn data_footage(services: &dyn crate::Services, path: &str) -> Option<std::result::Result<unwritten_studio_project::Footage, String>> {
    let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !DATA_EXTENSIONS.contains(&ext.as_str()) {
        return None;
    }
    Some((|| {
        let bytes = services.read_file(path).map_err(|e| e.to_string())?;
        let text = String::from_utf8(bytes).map_err(|_| "data files must be UTF-8 text".to_string())?;
        if ext == "json" {
            serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).map_err(|e| format!("invalid JSON: {e}"))?;
        }
        Ok(unwritten_studio_project::Footage {
            path: path.to_string(),
            kind: FootageKind::Data,
            codec: ext.to_uppercase(),
            data: Some(text),
            ..Default::default()
        })
    })())
}

fn project_settings(s: &mut Session, p: &Value) -> Result<Value> {
    s.edit("Project Settings", None, |proj, _| {
        if let Some(b) = str_p(p, "bitDepth") {
            proj.settings.bit_depth = match b {
                "8" | "8bpc" | "8 bpc" => unwritten_studio_project::BitDepth::Bpc8,
                "16" | "16bpc" | "16 bpc" => unwritten_studio_project::BitDepth::Bpc16,
                "32" | "32bpc" | "32 bpc" => unwritten_studio_project::BitDepth::Bpc32,
                _ => return Err(bad("file.projectSettings", "bitDepth must be 8, 16 or 32")),
            };
        } else if let Some(n) = p.get("bitDepth").and_then(Value::as_u64) {
            proj.settings.bit_depth = match n {
                16 => unwritten_studio_project::BitDepth::Bpc16,
                32 => unwritten_studio_project::BitDepth::Bpc32,
                _ => unwritten_studio_project::BitDepth::Bpc8,
            };
        }
        if let Some(l) = p.get("linearize").and_then(Value::as_bool) {
            proj.settings.linearize = l;
        }
        if let Some(l) = p.get("blendLinear").or_else(|| p.get("blendColorsUsing1Gamma")).and_then(Value::as_bool) {
            proj.settings.blend_linear = l;
        }
        use unwritten_studio_project::{ColorEngine, ColorSpace, HdrMode};
        let cmd = "file.projectSettings";
        if let Some(e) = str_p(p, "colorEngine") {
            let engine = match e.to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
                "adobe" | "adobemanaged" | "builtin" => ColorEngine::Adobe,
                "ocio" | "ociomanaged" => ColorEngine::Ocio,
                _ => return Err(bad(cmd, format!("colorEngine: adobe|ocio, not `{e}`"))),
            };
            if engine == ColorEngine::Ocio && proj.settings.color_engine != ColorEngine::Ocio {
                // The OCIO built-in config works in ACES and renders the display through its
                // tone-mapped view.
                if !proj.settings.working_space.is_some_and(|w| w.is_linear()) {
                    proj.settings.working_space = Some(ColorSpace::AcesCg);
                }
                if proj.settings.hdr == HdrMode::Clip {
                    proj.settings.hdr = HdrMode::ToneMap;
                }
            }
            proj.settings.color_engine = engine;
        }
        if let Some(w) = p.get("workingSpace") {
            let ids = ColorSpace::WORKING.iter().map(|c| c.id()).collect::<Vec<_>>().join("|");
            proj.settings.working_space = match w.as_str() {
                None | Some("none" | "None" | "") => None,
                Some(n) => Some(
                    ColorSpace::parse(n).filter(|c| ColorSpace::WORKING.contains(c)).ok_or_else(|| bad(cmd, format!("workingSpace: none|{ids}, not `{n}`")))?,
                ),
            };
        }
        if let Some(g) = p.get("workingGamma") {
            // After Effects takes 2.2 or 2.4 only (`app.project.workingGamma`).
            proj.settings.working_gamma = match g.as_f64() {
                Some(g) if (g - 2.2).abs() < 1e-6 => 2.2,
                Some(g) if (g - 2.4).abs() < 1e-6 => 2.4,
                _ => return Err(bad(cmd, format!("workingGamma: 2.2|2.4, not `{g}`"))),
            };
        }
        if proj.settings.color_engine == ColorEngine::Ocio && !proj.settings.working_space.is_some_and(|w| w.is_linear()) {
            return Err(bad(cmd, "the OCIO built-in config's working spaces are acescg and aces2065"));
        }
        if let Some(h) = str_p(p, "hdr") {
            proj.settings.hdr = match h.to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
                "clip" | "off" | "none" => HdrMode::Clip,
                "compand" => HdrMode::Compand,
                "tonemap" | "tonemapped" => HdrMode::ToneMap,
                _ => return Err(bad(cmd, format!("hdr: clip|compand|toneMap, not `{h}`"))),
            };
        }
        if let Some(o) = p.get("outputSpace") {
            proj.settings.output_space = match o.as_str() {
                None | Some("none" | "None" | "" | "default") => None,
                Some(n) => Some(ColorSpace::parse(n).ok_or_else(|| bad(cmd, format!("outputSpace: srgb|rec709|rec2020|p3|rec2100pq|rec2100hlg, not `{n}`")))?),
            };
        }
        if let Some(r) = p.get("renderer").or_else(|| p.get("gpuAcceleration")) {
            proj.settings.gpu_acceleration = match r {
                Value::Bool(b) => *b,
                Value::String(s) => match unwritten_studio_render::Backend::parse(s) {
                    Some(unwritten_studio_render::Backend::Cpu) => false,
                    Some(_) => true,
                    None => return Err(bad("file.projectSettings", format!("renderer: gpu|software, not `{s}`"))),
                },
                _ => return Err(bad("file.projectSettings", "renderer: gpu|software")),
            };
        }
        if let Some(t) = str_p(p, "timeDisplay") {
            proj.settings.time_display = unwritten_studio_project::TimeDisplayStyle::parse(t)
                .ok_or_else(|| bad("file.projectSettings", format!("timeDisplay: timecode|frames|feet35|feet16, not `{t}`")))?;
        }
        Ok(())
    })?;
    Ok(serde_json::to_value(&s.project.settings).unwrap_or_default())
}

/// Video Rendering and Effects: report or set the renderer (Mercury GPU Acceleration when a GPU
/// adapter exists, else Mercury Software Only).
fn render_backend(s: &mut Session, p: &Value) -> Result<Value> {
    if let Some(b) = p.get("backend").or_else(|| p.get("renderer")) {
        let gpu = match b {
            Value::Bool(b) => *b,
            Value::String(v) => match unwritten_studio_render::Backend::parse(v) {
                Some(unwritten_studio_render::Backend::Cpu) => false,
                Some(_) => true,
                None => return Err(bad("render.backend", format!("backend: gpu|cpu, not `{v}`"))),
            },
            _ => return Err(bad("render.backend", "backend: gpu|cpu")),
        };
        if gpu != s.project.settings.gpu_acceleration {
            s.edit("Project Settings", None, |proj, _| {
                proj.settings.gpu_acceleration = gpu;
                Ok(())
            })?;
        }
    }
    Ok(backend_status(s))
}

/// The renderer setting and what renders with it.
pub fn backend_status(s: &Session) -> Value {
    let adapter = s.accel.as_ref().map(|a| a.name());
    let gpu = s.project.settings.gpu_acceleration;
    // Why it renders on the CPU (#262).
    let why = if !gpu {
        Some("the project's renderer is Mercury Software Only".to_string())
    } else if adapter.is_none() {
        Some(s.accel_note.clone().unwrap_or_else(|| "no GPU compositor is attached".into()))
    } else {
        None
    };
    json!({
        "renderer": if gpu { "Mercury GPU Acceleration" } else { "Mercury Software Only" },
        "gpuAcceleration": gpu,
        "adapter": adapter,
        "active": if gpu && adapter.is_some() { "gpu" } else { "cpu" },
        "why": why,
    })
}

fn cycle_depth(s: &mut Session, _: &Value) -> Result<Value> {
    s.edit("Project Bit Depth", None, |proj, _| {
        proj.settings.bit_depth = proj.settings.bit_depth.next();
        Ok(())
    })?;
    Ok(json!(s.project.settings.bit_depth.label()))
}

fn export_after_effects(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").ok_or_else(|| bad("file.exportAfterEffects", "missing `path` (.jsx)"))?;
    if !path.to_ascii_lowercase().ends_with(".jsx") {
        return Err(bad("file.exportAfterEffects", "path must end in .jsx"));
    }
    let script = crate::ae_bridge::export_jsx(&s.project).map_err(EngineError::Other)?;
    s.services.write_file(path, script.as_bytes()).map_err(|e| EngineError::Other(format!("cannot write {path}: {e}")))?;
    Ok(json!({"path": path, "format": "jsx", "requiresAfterEffectsDesktop": true,
        "warnings": ["Run this JSX in After Effects desktop to reconstruct supported 2D layers and optionally save a new .aep. Unsupported features are reported; preserve the source project."]}))
}

fn export_after_effects_exporter(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").ok_or_else(|| bad("file.exportAfterEffectsExporter", "missing `path` (.jsx)"))?;
    if !path.to_ascii_lowercase().ends_with(".jsx") {
        return Err(bad("file.exportAfterEffectsExporter", "path must end in .jsx"));
    }
    s.services.write_file(path, crate::ae_bridge::EXPORTER.as_bytes()).map_err(|e| EngineError::Other(format!("cannot write {path}: {e}")))?;
    Ok(
        json!({"path": path, "format": "jsx", "requiresAfterEffectsDesktop": true, "instructions": "Run this exporter in AE desktop with your AEP open, then open its .unwae output in Unwritten Studio. Only the documented 2D subset is supported."}),
    )
}
pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("file.newProject", "New Project", ["File", "New"], Some("Cmd+Alt+N"), "{}", always, new_project),
        cmd!("file.openDemoProject", "Open Demo Project", ["Help"], None, "{}", always, demo),
        cmd!("file.open", "Open Project...", ["File"], Some("Cmd+O"), "{path}", always, open),
        cmd!(
            "file.exportAfterEffectsExporter",
            "Save AE Import Helper JSX...",
            ["File", "After Effects Bridge"],
            None,
            "{path: .jsx}",
            always,
            export_after_effects_exporter
        ),
        cmd!("file.exportAfterEffects", "Export Project to AE JSX...", ["File", "After Effects Bridge"], None, "{path: .jsx}", always, export_after_effects),
        cmd!("file.save", "Save", ["File"], Some("Cmd+S"), "{path?}", always, save),
        cmd!("file.saveAs", "Save As...", ["File", "Save As"], Some("Cmd+Shift+S"), "{path}", always, save_as),
        cmd!("file.incrementAndSave", "Increment and Save", ["File"], Some("Cmd+Alt+Shift+S"), "{}", has_path, increment_save),
        cmd!("file.revert", "Revert", ["File"], None, "{}", has_path, revert),
        cmd!(
            "file.import",
            "File...",
            ["File", "Import"],
            Some("Cmd+I"),
            "{paths: [string] (files or folders), sequence?: bool (the Sequence checkbox: true imports numbered stills of one run as one image sequence, one picked file bringing its whole run and several picked files that range; default: a numbered still brings its run, except dropped files (drag), which import as stills as in After Effects; folders' runs import as sequences; false imports stills and a folder's files individually), alphabetical?: bool (Force Alphabetical Order: every image of that type in the folder, by name), frameRate?: fps (image sequences; default Settings ▸ Import ▸ Sequence Footage), importAs?: footage|composition|compositionLayerSizes (Photoshop, PDF, Illustrator and EPS files), layer?: name|index (footage of one Photoshop layer), page?: number from 1 (PDF / Illustrator page), drag?: bool (dropped files: Settings ▸ Import ▸ Default Drag Import As; files import as stills unless sequence), addToComp?: bool (also add them to the active comp, at time?, index?, position? as in layer.addItem), background?: bool (probe the files in a background job, jobs.list / jobs.wait; returns {job})}",
            always,
            import_cmd
        ),
        cmd!(
            "file.projectSettings",
            "Project Settings...",
            ["File"],
            Some("Cmd+Alt+Shift+K"),
            "{bitDepth?: 8|16|32, colorEngine?: adobe|ocio, workingSpace?: none|srgb|rec709|rec2020|p3|acescg|aces2065, workingGamma?: 2.2|2.4 (with no working space), linearize?, blendLinear?, hdr?: clip|compand|toneMap, outputSpace?: srgb|rec709|rec2020|p3|rec2100pq|rec2100hlg, renderer?: gpu|software, timeDisplay?: timecode|frames|feet35|feet16}",
            always,
            project_settings
        ),
        cmd!("file.cycleBitDepth", "Cycle Project Bit Depth", [], None, "{}", always, cycle_depth),
        cmd!("render.backend", "Video Rendering and Effects", [], None, "{backend?: gpu|cpu}", always, render_backend),
    ]
}
