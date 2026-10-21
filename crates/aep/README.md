# unwritten-studio-aep

A bounded, read-only pure-Rust reader for the After Effects `.aep` container
format (RIFX/Egg), adapted from the clean-room `effectcraft-aep` reader in
[storytold/effectcraft PR #1378](https://github.com/storytold/effectcraft/pull/1378),
pinned to revision [`b8ed19ad689328d357db83237992fc09def3d9ff`](https://github.com/storytold/effectcraft/tree/b8ed19ad689328d357db83237992fc09def3d9ff).

This crate intentionally contains **reader sources only**. It does not include
Adobe software or artwork, exporter/template code, or binary fixtures. The
reader exposes a lossless chunk tree and typed, read-only accessors for verified
container fields (items, compositions, layers, transforms, properties,
keyframes, masks, shapes, text, effects, camera/light data, and footage).
Unknown or unverified fields remain opaque or absent; this is not a full-fidelity
After Effects implementation.

## Safety and limits

Input is untrusted. Parsing validates chunk sizes, nesting, arithmetic, and
configured resource limits before indexing; malformed or truncated data returns
an error or an empty/absent typed field rather than panicking. Use
`parse_with_limits` for app-controlled budgets, especially when opening files
from iOS document providers. The crate has no dependencies and is suitable for
native iOS integration; an application adapter should map the typed view into
`unwritten-studio-project` rather than exposing parser internals to the engine.

## Provenance and license

The implementation is derived from the public clean-room implementation in
Effectcraft PR #1378 under its repository's MIT OR Apache-2.0 project license.
The upstream work documents that its format observations came from generated
files and that its fixtures are synthetic. No upstream fixture or exporter is
included here. This crate remains MIT OR Apache-2.0 under the repository license.
