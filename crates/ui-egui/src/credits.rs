//! The About window's historical AI-model usage credits. No personal contributor data is bundled.

use crate::i18n::tr;
use egui::RichText;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub company: &'static str,
    pub model: &'static str,
    pub version: &'static str,
    pub commits: u64,
    pub lines_added: u64,
    pub lines_deleted: u64,
}

include!(concat!(env!("OUT_DIR"), "/credits.rs"));

fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// About ▸ Models: AI models recorded in the project's historical development notes.
pub fn models_ui(ui: &mut egui::Ui) {
    if MODELS.is_empty() {
        ui.label(tr("No model credits were built into this copy."));
        return;
    }
    let assisted: u64 = MODELS.iter().map(|m| m.commits).max().unwrap_or(0).max(1);
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("credits_models").striped(true).num_columns(6).show(ui, |ui| {
            for h in ["Company", "Model", "Version", "Commits", "% of all commits", "Lines +/−"] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for m in MODELS {
                ui.label(m.company);
                ui.label(m.model);
                ui.label(m.version);
                ui.label(group(m.commits));
                ui.label(format!("{:.0}%", 100.0 * m.commits as f64 / TOTAL_COMMITS.max(assisted) as f64));
                ui.label(format!("+{} / −{}", group(m.lines_added), group(m.lines_deleted)));
                ui.end_row();
            }
        });
    });
}
