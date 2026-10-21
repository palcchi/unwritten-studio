# Saving an After Effects AEP copy

Unwritten Studio can export an experimental **real binary `.aep` file**. This is a big-endian `RIFX` container with the `Egg!` form type, not native JSON, XML, or a JSX script renamed to `.aep`. The Rust writer runs locally, including in the iOS build; it needs neither Python nor an After Effects installation to generate the file.

Keep your editable master as **`.unw`**. Exporting an AEP is a separate copy operation: it must not change the current native document path, clear unsaved edits, or overwrite an imported source project. Native Save / Save As still writes `.unw`; it never disguises native data as an Adobe project.

## Target version

The requested target is the latest stable After Effects, not the beta. [Adobe's release notes](https://helpx.adobe.com/after-effects/desktop/what-s-new/release-notes-after-effects.html) identify **After Effects 2026 version 26.5 (September 2026)** as the latest stable release, verified October 21, 2026. The writer uses the existing AE2026 synthetic structures; selecting 26.5 does not mean changing opaque binary version fields without evidence. Opening, saving, and reopening this fork's output in the actual AE 26.5 application remains an outstanding acceptance test, not a claimed result.

## What transfers

The initial writer exports folders, compositions, solid/adjustment/null layers, supported 2D transform values and keyframes, temporal interpolation/ease, supported spatial tangents, transform expression source, supported layer flags, and parent relationships between exported layers. Stored expression source does not guarantee identical evaluation in the two applications.

It does **not** export arbitrary projects faithfully. Text, shapes, footage/audio, precomposition layers, cameras/lights, effects, masks, markers, track mattes, time remapping and advanced 3D/project settings are outside this writer's scope. Time stretch and some unsupported settings or values require approximation. Every export returns an explicit loss report. The default command refuses detected losses before writing a file; only an explicit choice permits a partial copy. Save the native project and retain original media and the original AEP.

An imported AEP has already passed through a partial importer. Its export report describes losses from the **current native document**, not features previously lost while importing. An export report with no new losses is therefore not evidence of a faithful round trip of the original AEP.

## Command interface

`file.exportAep` accepts `path`, optional `dryRun`, and optional `allowLosses` (default `false`). Use a fresh `.aep` destination. A dry run returns the report without writing. Existing destinations are refused; choose a new filename. A failed or refused export must leave the current edit and existing destination unchanged. The container writer preflights all sizes and nesting before reserving output memory, and limits output including metadata to 64 MiB.

On native hosts, publication uses a fully written, flushed, exclusive staging file and a same-directory hard link that cannot replace an existing destination. Another process creating the destination after preflight therefore cannot cause an overwrite; existing symlinks are refused too. Filesystems that do not support hard links return an error instead of falling back to an overwriting rename: export to local storage first. Browser storage reserves the normalized destination under its store lock. Custom hosts must explicitly implement the new-file service; an ordinary overwrite-capable writer is not used as a fallback.

The browser prepares the download before claiming the new storage path, then starts it only after the exclusive write succeeds. A reported preparation failure therefore leaves the name available for retry, and a storage collision never starts a download. Browsers do not report whether the user ultimately saves or cancels an offered download; a successful command means the copy is stored and the download was offered, not that a file was confirmed on the user's disk.

```json
{"command":"file.exportAep","params":{"path":"My animation.aep","dryRun":true}}
```

Only after reviewing omissions, a caller can explicitly request `allowLosses: true`. Do not use this option automatically on a user's real project.

## Why this is a genuine AEP writer

The implementation is adapted from the permissively licensed [EffectCraft exporter, PR #1422](https://github.com/vangel76/effectcraft/pull/1422), pinned to `02b490ef95dd345b098822367847cbfdfb80da48`. It edits consistent chunk structures using two original contributor-authored, openly licensed synthetic project/prototype files. Their sidecars describe their origin; they contain no Adobe sample project, third-party artwork, media, preset, or application code. The user's private AEP is never embedded or distributed.

The [upstream read-back notes](https://github.com/vangel76/effectcraft/blob/02b490ef95dd345b098822367847cbfdfb80da48/docs/aep-export.md#after-effects-read-back) report opening generated files in After Effects 2026, reading back composition/layer/property values, and re-saving selected files. That is upstream evidence, **not a new test of this fork**. Local parser round trips, independent parser checks and Linux tests do not certify opening in After Effects or visual fidelity. No compatibility guarantee is made for other AE versions.

## Reproduce local checks

```sh
cargo test -p unwritten-studio-aep -p unwritten-studio-aep-export
cargo test -p unwritten-studio-engine aep
cargo run -p unwritten-studio-engine --example aep_exchange_check -- /tmp/new-aep-check-folder
```

The example creates an original small solid/null animation, saves a `.unw` master, exports an AEP through the real command, then reopens both and checks that the export did not alter the current document. The output directory must not already exist. It can also read a private source AEP using the example's second argument; that mode tests dry-run/default refusal only and never exports a lossy copy automatically.

It also retries the export to the existing AEP and to the native master: both must be refused without changing either file or clearing the unsaved document state. Reopened native and AEP files are checked for the demo's position keyframes and opacity, not merely their composition and layer counts.

No new IPA, iOS simulator build, physical-device test, or local After Effects desktop test is implied by adding this source feature.
