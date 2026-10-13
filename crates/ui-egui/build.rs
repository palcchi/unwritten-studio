//! Bake historical model-usage credits into the binary as a static Rust table.
//! Personal contributor records are intentionally not bundled or displayed.

use serde_json::Value;
use std::fmt::Write as _;

fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../credits/models.json");
    println!("cargo::rerun-if-changed={}", path.display());
    let data = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string()))
        .unwrap_or_else(|e| {
            println!("cargo::warning={}: {e}; the About window shows no model credits", path.display());
            Value::Null
        });
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("credits.rs");
    if let Err(e) = std::fs::write(out, generate(&data)) {
        println!("cargo::warning=writing generated model credits: {e}");
    }
}

fn n(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn s(v: &Value, key: &str) -> String {
    format!("{:?}", v.get(key).and_then(Value::as_str).unwrap_or(""))
}

fn generate(data: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "pub static TOTAL_COMMITS: u64 = {};", n(data, "total_commits"));
    let _ = writeln!(out, "pub static MODELS: &[Model] = &[");
    for model in data.get("models").and_then(Value::as_array).into_iter().flatten() {
        let _ = writeln!(
            out,
            "    Model {{ company: {}, model: {}, version: {}, commits: {}, lines_added: {}, lines_deleted: {} }},",
            s(model, "company"),
            s(model, "model"),
            s(model, "version"),
            n(model, "commits"),
            n(model, "lines_added"),
            n(model, "lines_deleted"),
        );
    }
    let _ = writeln!(out, "];\n");
    out
}
