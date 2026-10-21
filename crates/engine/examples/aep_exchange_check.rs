//! Exercise native .unw save and binary .aep export through the real commands.
//! All generated assets are original procedural solids. Private inputs are read-only.
use serde_json::json;
use std::path::Path;
use unwritten_studio_engine::Session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let output = args.next().ok_or("usage: aep_exchange_check NEW_OUTPUT_DIRECTORY [PRIVATE_SOURCE.aep]")?;
    let private_source = args.next();
    // create_dir, not create_dir_all: an existing destination must never be reused.
    std::fs::create_dir(&output)?;
    let native_path = Path::new(&output).join("Unwritten-AEP-demo.unw");
    let aep_path = Path::new(&output).join("Unwritten-AEP-demo.aep");
    let mut session = Session::default();
    session.execute("comp.new", json!({"name":"Unwritten export demo","width":640,"height":360,"duration":3,"frameRate":30}))?;
    let layer = session.execute("layer.newSolid", json!({"name":"Original violet solid","color":"#7c3aed","width":160,"height":100}))?["layer"]
        .as_u64()
        .ok_or("new solid returned no layer id")?;
    session.execute("prop.addKey", json!({"layer":layer,"path":"transform/position","time":0,"value":[100,180,0]}))?;
    session.execute("prop.addKey", json!({"layer":layer,"path":"transform/position","time":2,"value":[540,180,0]}))?;
    session.execute("layer.newNull", json!({"name":"Control null"}))?;
    session.execute("file.saveAs", json!({"path":native_path}))?;
    // Leave a genuine unsaved edit: exporting must not clear it or change the native path.
    session.execute("prop.set", json!({"layer":layer,"path":"transform/opacity","value":85}))?;
    let before = session.project.to_json();
    let before_path = session.path.clone();
    let before_dirty = session.is_dirty();
    let dry = session.execute("file.exportAep", json!({"path":aep_path,"dryRun":true}))?;
    if aep_path.exists() {
        return Err("dry run wrote a destination".into());
    }
    let exported = session.execute("file.exportAep", json!({"path":aep_path}))?;
    if session.project.to_json() != before || session.path != before_path || session.is_dirty() != before_dirty || !before_dirty {
        return Err("export changed the native document or its unsaved state".into());
    }
    // After proving export did not save the native document, save the matching master explicitly.
    session.execute("file.save", json!({}))?;
    let bytes = std::fs::read(&aep_path)?;
    if bytes.get(..4) != Some(b"RIFX") || bytes.get(8..12) != Some(b"Egg!") {
        return Err("export is not a binary AEP container".into());
    }
    let mut reopened = Session::default();
    let imported = reopened.execute("file.open", json!({"path":aep_path}))?;
    let layers: usize = reopened.project.comps().map(|(_, c)| c.layers.len()).sum();
    if reopened.project.comps().count() != 1 || layers != 2 {
        return Err("export did not preserve the generated composition and two layers".into());
    }
    let mut native_reopened = Session::default();
    native_reopened.execute("file.open", json!({"path":native_path}))?;
    if native_reopened.project.comps().count() != 1 {
        return Err("native .unw did not reopen".into());
    }
    let mut private_result = serde_json::Value::Null;
    if let Some(source) = private_source {
        let original = std::fs::read(&source)?;
        let mut private = Session::default();
        let opened = private.execute("file.open", json!({"path":source}))?;
        let destination = Path::new(&output).join("private-source-NOT-WRITTEN.aep");
        let analyzed = private.execute("file.exportAep", json!({"path":destination,"dryRun":true}))?;
        let refusal = private.execute("file.exportAep", json!({"path":destination})).err().map(|e| e.to_string());
        if destination.exists() || std::fs::read(&source)? != original {
            return Err("private source check wrote data unexpectedly".into());
        }
        private_result = json!({"sourceUnchanged":true,"importReport":opened["report"],"exportDryRun":analyzed,"defaultRefusal":refusal});
    }
    let result = json!({"nativeFile":native_path,"aepFile":aep_path,"binaryAep":true,"exportPreservesUnsavedNativeDocument":true,
        "nativeReopen":true,"aepReimportCompositions":1,"aepReimportLayers":layers,"dryRun":dry,"export":exported,"reimportReport":imported["report"],
        "privateSource":private_result,"afterEffectsDesktopTestedLocally":false});
    let report_path = Path::new(&output).join("exchange-check.json");
    std::fs::write(&report_path, serde_json::to_vec_pretty(&result)?)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
