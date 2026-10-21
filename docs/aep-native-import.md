# Opening After Effects AEP files directly

Unwritten Studio now has an experimental **native, read-only binary `.aep` importer**. The Rust parser runs inside the app, including iOS. After Effects desktop, Python, a server, and the `.unwae` conversion bridge are not required to read a supported AEP container.

Choose **File > Open Project** and select an `.aep` in Files. On iOS, a warning asks whether to open an experimental copy. The imported document is **untitled and unsaved**: save a new `.unw` copy. The original AEP is never modified. File > Import Project merges the supported content into the current project as one undoable edit.

## What opens

The current adapter maps the project item tree, composition settings, solid/null/adjustment/footage/precomp layers, timing, parent relationships, supported blend modes, transform values and keyframes, and stored expressions. Referenced media remains linked, not embedded; transfer and relink unavailable media. Expression source being imported is not certification of identical AE evaluation.

This is **not full After Effects compatibility**. The initial adapter reports unsupported content, including text/shape/camera/light layers, effects, masks, markers, unsupported transform streams, and approximated keys. Track mattes, layer styles, frame blending, audio levels, and other undecoded features are also listed as standing limitations. A composition opening does not prove its rendered output matches AE.

Third-party effects such as Sapphire (`S_*`) or Boris Continuum (`BCC *`) can be recorded in an otherwise readable AEP. They are dependencies of the original AE project, not proof that its container is corrupt. This adapter currently omits **all** effects, including AE built-ins, and names detected effects in its report. Installing a desktop AE plugin does not make that plugin run inside Unwritten Studio or its iOS build. Missing footage is a separate relinking issue. Other unsupported layers and properties can still be lost even when every plugin is available in AE.

The command result from `file.open` includes `format: "aep"`, the full `report`, and `warnings`. `file.open` also accepts `allowLosses: false` to refuse a project with detected losses without replacing the current document. Normal UI opening imports a partial copy with explicit warnings. No source file is overwritten on failure or success. Malformed input fails before session replacement; files with no readable compositions are refused rather than presenting an invented empty project.

## Export and remaining limits

- A separate experimental [binary `.aep` exporter](aep-native-export.md) now writes a narrow supported subset directly. It is not a faithful reversal of this importer: unsupported original content is not recovered. Save edits as `.unw`; retain the original AEP. The JSX desktop bridge remains an alternative with a different supported subset.
- `.aepx` XML import remains unsupported. Native Unwritten XML is a different format.
- Direct Files-app launching into the editor and persistent security-scoped media access are separate host integration work; use the in-app Files picker.
- No claim of simulator, signed physical-device, or real-AE visual certification is implied by Linux tests.

## Source, permission, and limits

The repository owner explicitly authorized reverse engineering of the AEP format and reuse of GitHub implementations on 21 October 2026, then explicitly requested directly openable binary export. This is the local fork's decision for the previously blocked G4 workstream, not an upstream maintainer decision. The scope is file-format work and permissively licensed code, not disassembly/copying of Adobe program code, artwork, presets, or Adobe sample projects. The separate exporter uses openly licensed contributor-authored synthetic structures; see its provenance.

The parser and adapter are adapted from [EffectCraft PR #1378](https://github.com/storytold/effectcraft/pull/1378), pinned to `b8ed19ad689328d357db83237992fc09def3d9ff`, under MIT OR Apache-2.0. The reader's upstream provenance describes one-change-per-file observations. Existing notices and licenses remain intact. See [`crates/aep/PROVENANCE.md`](../crates/aep/PROVENANCE.md) and the adapter's provenance. The reader adaptation bundles no Adobe program code, binary fixture corpus, or user-supplied project. The separate exporter has its own explicitly attributed synthetic structures.

The input limit is 64 MiB, with bounded chunk count and nesting. Unsupported/unknown data is not treated as fully understood. Imported projects pass the native project validation/repair boundary before becoming the current document.

## Reproduce the read-only workflow check

```sh
cargo test -p unwritten-studio-aep
cargo test -p unwritten-studio-aep-import
cargo run -p unwritten-studio-engine --example aep_native_check -- /path/to/private-project.aep
```

The example checks native File Open, an active composition, untitled/dirty state, native project serialization, project import/undo, and unchanged source bytes. It prints a report without embedding or redistributing the source. The private supplied sample stays outside the repository.
