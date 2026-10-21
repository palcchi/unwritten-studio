# AEP reader provenance

- Upstream project: [storytold/effectcraft](https://github.com/storytold/effectcraft)
- Upstream change: [PR #1378](https://github.com/storytold/effectcraft/pull/1378)
- Pinned source revision: `b8ed19ad689328d357db83237992fc09def3d9ff`
- Scope copied: `crates/aep/src/*.rs` reader and typed-view implementation only.
- Explicitly excluded: `crates/aep/tests/` fixtures, exporter/writer templates,
  Adobe program code, and artwork.
- Adaptation: package renamed to `unwritten-studio-aep`; no external dependencies.

The upstream PR describes a clean-room process based on generated observations.
This repository makes no claim that the reader supports every After Effects
version, feature, or effect. Callers must treat all `.aep` input as untrusted
and surface unsupported/unknown fields to users.

## Writer follow-up

The container writer is adapted from `crates/aep/src/write.rs` at `vangel76/effectcraft` revision `02b490ef95dd345b098822367847cbfdfb80da48` (PR #1422), MIT OR Apache-2.0. Local hardening adds a checked complete-size preflight, a 64 MiB output cap and fallible exact reservation before copying any chunk data. Synthetic padding, depth, output-I/O-error and oversized borrowed-tree regression tests contain no private or Adobe fixture data. The separate exporter records its own template provenance in `../aep-export/PROVENANCE.md`.
