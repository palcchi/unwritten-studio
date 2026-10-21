//! Opt-in local inspection of an After Effects file.
//!
//! Run with `cargo run -p unwritten-studio-aep-import --example inspect_aep -- /path/file.aep`.
//! This reports the importer model and losses; it does not claim pixel or feature fidelity.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("usage: inspect_aep <file.aep>")?;
    let bytes = fs::read(&path)?;
    let report = unwritten_studio_aep_import::analyze(&bytes, &|p| Path::new(p).exists())?;
    println!("file: {path}");
    println!("compositions: {}", report.imported.compositions);
    println!("layers: {}", report.imported.layers);
    println!("losses: {}", report.total_losses());
    for loss in &report.losses {
        println!("  {}: {}", loss.category.label(), loss.count);
        for detail in &loss.details {
            println!("    - {detail}");
        }
    }
    if !report.missing_footage.is_empty() {
        println!("missing footage: {}", report.missing_footage.len());
    }
    println!("limitations: {}", report.limitations.len());
    Ok(())
}
