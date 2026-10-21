//! After Effects project (`.aep`) import: the typed view of [`unwritten_studio_aep`] mapped into an
//! Unwritten Studio [`Project`].
//!
//! What arrives: the project item tree (folders), compositions (size, exact frame rate, duration,
//! work area, background, pixel aspect, motion blur, shutter angle, renderer), solid, null,
//! adjustment, footage and precomposition layers (name, timing, stretch, switches, label, blend
//! mode for the six sampled modes, parent), the Transform properties (static values and keyframes
//! with interpolation, ease and spatial tangents) and their expressions as source text, solids,
//! and footage files as links (missing files become missing-footage placeholders).
//!
//! What does not arrive is **reported**, never dropped silently: [`ImportReport`] counts effects,
//! masks, markers, shape/text/camera/light layers, unsupported blend modes and transform
//! properties, simplified keyframes and unreadable data per [`LossCategory`]. By default the import
//! is refused when anything would be lost ([`ImportOptions::allow_losses`] lifts that); a refusal
//! changes nothing. A damaged file yields an error or a report, never a panic.
//!
//! The crate sits at L3 (above `unwritten-studio-project`, as `unwritten-studio-lottie` does) so it builds
//! for wasm32; it reads no files itself: the caller passes the bytes and a way to ask whether a
//! footage path exists.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod import;
mod props;
mod report;
mod time;

use unwritten_studio_project::{ItemId, Project};

pub use report::{ImportReport, Imported, LIMITATIONS, Loss, LossCategory, MAX_DETAILS};

/// Largest input accepted by the importer.
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

const PARSE_LIMITS: unwritten_studio_aep::Limits = unwritten_studio_aep::Limits { max_depth: 64, max_chunks: 250_000 };

/// Why an `.aep` was not imported.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AepImportError {
    /// The file is not a readable After Effects project container.
    #[error("{0}")]
    Container(#[from] unwritten_studio_aep::AepError),
    /// The container has no project (no `Fold` list).
    #[error("the file holds no After Effects project")]
    NoProject,
    /// The caller supplied a file larger than the importer safety bound.
    #[error("After Effects file is too large ({size} bytes; maximum is {max} bytes)")]
    FileTooLarge { size: usize, max: usize },
    /// Parts of the project cannot be imported and losses were not allowed. The project is
    /// unchanged; the report says what would be lost.
    #[error("importing would lose {}; allow losses to import anyway", .0.summary())]
    Refused(Box<ImportReport>),
}

/// Options of [`import`].
pub struct ImportOptions<'a> {
    /// Import even when [`ImportReport::losses`] is not empty.
    pub allow_losses: bool,
    /// Whether a footage path exists (the caller's file access); a path that does not becomes a
    /// missing-footage placeholder.
    pub exists: &'a dyn Fn(&str) -> bool,
    /// The folder every imported item goes into (After Effects names it after the file).
    pub folder_name: &'a str,
}

impl<'a> ImportOptions<'a> {
    /// Losses refused, folder `After Effects Project`.
    pub fn new(exists: &'a dyn Fn(&str) -> bool) -> Self {
        Self { allow_losses: false, exists, folder_name: "After Effects Project" }
    }
}

/// What an import created.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportResult {
    /// The folder holding the imported items.
    pub folder: ItemId,
    /// The compositions, in file order.
    pub comps: Vec<ItemId>,
    /// Every item created, the folder first.
    pub items: Vec<ItemId>,
    pub report: ImportReport,
}

/// Imports the `.aep` in `bytes` into `project` (as new items; nothing existing changes).
///
/// # Errors
/// [`AepImportError::Container`] / [`AepImportError::NoProject`] for a file that is not a project,
/// [`AepImportError::Refused`] when something would be lost and `allow_losses` is not set. On any
/// error `project` is unchanged.
pub fn import(project: &mut Project, bytes: &[u8], options: &ImportOptions<'_>) -> Result<ImportResult, AepImportError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(AepImportError::FileTooLarge { size: bytes.len(), max: MAX_FILE_BYTES });
    }
    let file = unwritten_studio_aep::parse_with_limits(bytes, PARSE_LIMITS)?;
    let view = unwritten_studio_aep::Project::new(&file).ok_or(AepImportError::NoProject)?;
    // Work on a copy so a refusal leaves the project untouched (compositions are shared, cheap).
    let mut work = project.clone();
    let (folder, comps, items, report) = import::run(&mut work, &view, options);
    if !options.allow_losses && !report.is_lossless() {
        return Err(AepImportError::Refused(Box::new(report)));
    }
    work.fix_next_id();
    *project = work;
    Ok(ImportResult { folder, comps, items, report })
}

/// The report an import would produce, without touching any project (a dry run).
///
/// # Errors
/// As [`import`], except that losses are never refused.
pub fn analyze(bytes: &[u8], exists: &dyn Fn(&str) -> bool) -> Result<ImportReport, AepImportError> {
    let mut scratch = Project::default();
    let options = ImportOptions { allow_losses: true, exists, folder_name: "After Effects Project" };
    Ok(import(&mut scratch, bytes, &options)?.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Imported, Loss};

    #[test]
    fn options_default_to_refusing_losses() {
        let exists = |_: &str| false;
        let options = ImportOptions::new(&exists);
        assert!(!options.allow_losses);
        assert_eq!(options.folder_name, "After Effects Project");
    }

    #[test]
    fn report_summary_and_counts_are_deterministic() {
        let report = ImportReport {
            imported: Imported::default(),
            losses: vec![Loss { category: LossCategory::Effects, count: 2, details: Vec::new() }],
            missing_footage: Vec::new(),
            limitations: Vec::new(),
        };
        assert_eq!(report.total_losses(), 2);
        assert_eq!(report.count(LossCategory::Effects), 2);
        assert_eq!(report.summary(), "effects: 2");
        assert!(!report.is_lossless());
    }

    #[test]
    fn malformed_bytes_return_an_error_without_mutating_project() {
        let exists = |_: &str| false;
        let mut project = Project::default();
        let result = import(&mut project, b"not an aep", &ImportOptions::new(&exists));
        assert!(matches!(result, Err(AepImportError::Container(_))));
        assert!(project.items.is_empty());
    }
}
