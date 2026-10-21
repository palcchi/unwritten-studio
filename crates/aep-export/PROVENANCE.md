# Native AEP exporter provenance

- Upstream project: [vangel76/effectcraft](https://github.com/vangel76/effectcraft)
- Upstream change: PR #1422
- Pinned source revision: `02b490ef95dd345b098822367847cbfdfb80da48`
- Pinned source URL: https://raw.githubusercontent.com/vangel76/effectcraft/02b490ef95dd345b098822367847cbfdfb80da48/
- Scope adapted: `crates/aep-export/src/*.rs`, `tests/export.rs`, `tests/robustness.rs`, `tests/common.rs`, `template/base.aep`, `template/protos.aep`, plus the native `crates/aep/src/write.rs` and its required error/API additions.
- Package adaptation: `effectcraft-*` identifiers were renamed to the local `unwritten-studio-*` conventions. The exporter remains pure Rust and performs no file I/O.
- Explicitly excluded: corpus-based roundtrip tests and all unrelated upstream crates, Adobe application code, Adobe sample projects, third-party media, presets, templates, and personal data.

## Observations and local verification

The exporter design and the template chunk layouts are **upstream AE2026 observations** documented by the contributor: the templates are generated synthetic project structures and contain no Adobe sample project or media. The `.aep` templates are retained under the contributor's MIT OR Apache-2.0 project licence; their attribution sidecars are adjacent to each asset.

The following are **local tests**, not claims of universal After Effects compatibility: `cargo test -p unwritten-studio-aep -p unwritten-studio-aep-export` validates deterministic RIFX/Egg! output, parser/writer consistency, narrow solids/nulls/2D-transform export, loss refusal by default, malformed-input robustness, bounded project traversal, and hostile numeric/name inputs. Unsupported features are reported and exports that would lose data are refused unless explicitly allowed.
