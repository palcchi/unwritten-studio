//! Native After Effects export review. The engine remains the source of truth;
//! this panel only gates the interactive file-picker path.
use crate::UnwrittenStudioApp;
use egui::Context;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Review {
    pub report: Value,
}

pub fn show(app: &mut UnwrittenStudioApp, ctx: &Context) {
    let Some(review) = app.aep_export_review.as_ref() else { return };
    let report = review.report.clone();
    let losses = report.get("losses").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut action: Option<bool> = None;
    let modal = egui::Modal::new(egui::Id::new("aep-export-review-modal")).show(ctx, |ui| {
        ui.set_width((ctx.content_rect().width() - 40.0).clamp(220.0, 560.0));
        ui.heading("Export After Effects AEP");
        ui.label("Experimental native AEP export");
        ui.label("Target: After Effects 2026 (26.5, latest stable verified October 21, 2026). The writer uses AE2026 structures; this build has not been opened in AE 26.5, so compatibility is not yet verified. Your native .unw project is kept unchanged.");
        ui.separator();
        if losses.is_empty() {
            ui.label("Preflight: no losses detected. The supported subset can be exported as-is.");
        } else {
            ui.colored_label(egui::Color32::YELLOW, format!("Preflight: {} loss categories detected.", losses.len()));
            ui.label("The following features will be omitted or approximated:");
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for loss in &losses {
                    let category = loss.get("category").and_then(Value::as_str).unwrap_or("unknown");
                    let count = loss.get("count").and_then(Value::as_u64).unwrap_or(0);
                    ui.label(format!("• {category}: {count}"));
                    if let Some(details) = loss.get("details").and_then(Value::as_array) {
                        for detail in details.iter().take(3).filter_map(Value::as_str) {
                            ui.small(format!("  {detail}"));
                        }
                    }
                }
            });
            ui.label("Exporting partial AEP is explicit and may change the result when opened in After Effects.");
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.push_id("aep-export-cancel", |ui| {
                let response = ui.button("Cancel");
                app.auto.add_response("file.exportAep.review.cancel", &response, "Cancel");
                if response.clicked() {
                    action = Some(false);
                }
            });
            if losses.is_empty() {
                ui.push_id("aep-export-confirm", |ui| {
                    let response = ui.button("Export AEP");
                    app.auto.add_response("file.exportAep.review.export", &response, "Export AEP");
                    if response.clicked() {
                        action = Some(true);
                    }
                });
            } else {
                ui.push_id("aep-export-partial", |ui| {
                    let response = ui.button("Export partial AEP");
                    app.auto.add_response("file.exportAep.review.partial", &response, "Export partial AEP");
                    if response.clicked() {
                        action = Some(true);
                    }
                });
            }
        });
    });
    app.auto.add_response("file.exportAep.review.backdrop", &modal.backdrop_response, "AEP export review");
    if action.is_none() && modal.should_close() {
        action = Some(false);
    }
    if let Some(accept) = action {
        app.aep_export_review = None;
        if accept {
            let params = serde_json::json!({"allowLosses": !losses.is_empty()});
            if let Err(error) = crate::menus::invoke(app, ctx, "file.exportAep", params) {
                app.ui.status = error;
            }
        }
    }
}
