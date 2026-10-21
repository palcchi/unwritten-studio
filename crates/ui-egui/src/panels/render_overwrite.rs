//! One-shot render replacement approval. Enter never approves a destructive replacement.
use crate::{Dialog, UnwrittenStudioApp, theme::Tokens};
use egui::vec2;
use serde_json::{Value, json};

#[derive(Clone, Debug, Default)]
pub struct Pending {
    command: Option<(Value, Vec<String>)>,
}
pub fn guard(app: &mut UnwrittenStudioApp, id: &str, params: &Value) -> bool {
    if id != "renderQueue.render" || params.get("overwrite").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    let paths = app.session.render_output_collisions();
    if paths.is_empty() {
        return false;
    }
    app.dialog_state.render_overwrite.command = Some((params.clone(), paths));
    app.dialog = Some(Dialog::RenderOverwrite);
    true
}
pub fn show(app: &mut UnwrittenStudioApp, ctx: &egui::Context, t: &Tokens) {
    let Some((params, paths)) = app.dialog_state.render_overwrite.command.clone() else {
        app.dialog = None;
        return;
    };
    let mut answer = None;
    super::dialogs::modal(ctx, "Replace render output?", vec2(520.0, 300.0), t, |ui| {
        ui.label(format!("{} existing output file(s) will be replaced. This cannot be undone.", paths.len()));
        egui::ScrollArea::vertical().max_height(130.0).show(ui, |ui| {
            for path in paths.iter().take(100) {
                ui.label(path);
            }
            if paths.len() > 100 {
                ui.label(format!("…and {} more", paths.len() - 100));
            }
        });
        ui.label("Cancel and choose a different Output To name or folder to keep these files.");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let cancel = ui.button("Cancel");
            app.auto.add("dialog.renderOverwrite.cancel", cancel.rect, "Cancel");
            if cancel.clicked() {
                answer = Some(false);
            }
            let replace = ui.add(egui::Button::new("Replace and Render").fill(t.danger));
            app.auto.add("dialog.renderOverwrite.replace", replace.rect, "Replace and Render");
            if replace.clicked() {
                answer = Some(true);
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        answer = Some(false);
    }
    let Some(replace) = answer else { return };
    app.dialog = None;
    app.dialog_state.render_overwrite.command = None;
    if !replace {
        return;
    }
    // A different queue/path set requires fresh approval, not a stale blanket overwrite.
    if app.session.render_output_collisions() != paths {
        let _ = guard(app, "renderQueue.render", &params);
        app.ui.status = "Output files changed. Review the replacement list again.".into();
        return;
    }
    let mut p = if params.is_object() { params } else { json!({}) };
    p["overwrite"] = json!(true);
    p["wait"] = json!(false);
    if let Err(error) = crate::menus::run_engine(app, ctx, "renderQueue.render", p) {
        app.ui.status = error;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_output_requires_one_shot_confirmation() {
        let path = std::env::temp_dir().join(format!("unwritten-ui-overwrite-{}.mp4", std::process::id()));
        std::fs::write(&path, b"keep original").unwrap();
        let mut app = UnwrittenStudioApp::new(Default::default());
        app.session.execute("comp.new", json!({"width":32,"height":24,"duration":1})).unwrap();
        app.session.execute("renderQueue.add", json!({"output":path.to_string_lossy(),"format":"h264"})).unwrap();
        assert!(guard(&mut app, "renderQueue.render", &json!({"wait":false})));
        assert!(matches!(app.dialog, Some(Dialog::RenderOverwrite)));
        assert!(app.dialog_state.render_overwrite.command.is_some());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep original");
        assert!(!guard(&mut app, "renderQueue.render", &json!({"overwrite":true})));
        let _ = std::fs::remove_file(path);
    }
}
