//! Read-only native AEP workflow check. Pass a private local AEP path explicitly.
//! No supplied project bytes, names, or media are embedded in this example.
use serde_json::json;
use unwritten_studio_engine::Session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: cargo run -p unwritten-studio-engine --example aep_native_check -- /path/to/project.aep")?;
    let original = std::fs::read(&path)?;
    let mut session = Session::default();
    let result = session.execute("file.open", json!({"path":path}))?;
    if session.path.is_some() || !session.is_dirty() || session.state.active_comp.is_none() {
        return Err("native AEP must open as an active, unsaved native copy".into());
    }
    let comps = session.project.comps().count();
    let layers: usize = session.project.comps().map(|(_, c)| c.layers.len()).sum();
    if comps == 0 {
        return Err("no compositions imported".into());
    }
    let native = session.project.to_json();
    let copy = unwritten_studio_project::Project::from_json(&native)?;
    if copy.comps().count() != comps {
        return Err("native project round-trip changed composition count".into());
    }
    // Import into an existing edit and undo it without losing the earlier document.
    let mut destination = Session::default();
    destination.execute("comp.new", json!({"name":"Existing edit","width":32,"height":24,"duration":1}))?;
    let before = destination.project.to_json();
    destination.execute("file.importProject", json!({"path":path}))?;
    destination.execute("edit.undo", json!({}))?;
    if destination.project.to_json() != before {
        return Err("AEP project import was not one reversible edit".into());
    }
    if std::fs::read(&path)? != original {
        return Err("source AEP changed".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"nativeOpen":true,"activeComp":true,"untitledDirtyCopy":true,"compositions":comps,"layers":layers,"nativeRoundTrip":true,"importUndo":true,"sourceUnchanged":true,"report":result["report"],"warnings":result["warnings"]})
        )?
    );
    Ok(())
}
