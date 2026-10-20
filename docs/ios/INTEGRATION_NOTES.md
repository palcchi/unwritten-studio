# iOS integration notes

## The original editor owns the workspace

`OriginalWorkspaceView` hosts `UnwrittenStudioApp::embedded_logic` and `embedded_ui` from the existing Rust egui frontend. Its menus, docks, timeline, composition viewer, effects and command dispatch are not recreated in SwiftUI. The older `StudioModel`, `WorkspaceView` and `ProjectDocument` files are historical implementation material, not the running editor's UI or document model.

The UIKit worker serializes access to its original-editor handle. It forwards JSON pointer, key, text, paste and file-dialog events, then presents RGBA frames produced by the original egui/wgpu renderer. This copied-frame presenter is not a zero-copy Metal surface and does not establish a frame-rate or device-performance guarantee.

## Documents and media

The original frontend issues file-dialog requests through its asynchronous hook. UIKit supplies native document pickers and returns paths to the same Rust callbacks used by the frontend. Original engine commands remain responsible for parsing, editing, saving and export.

Picked individual media files are copied into app-owned Application Support storage before their paths are returned to the engine. Security-scoped access is held only while reading the source. Destination collisions receive distinct names, and copy failures are reported rather than silently importing unavailable provider paths.

Projects and export folders keep security-scoped access for the current controller lifetime. They are not relocated automatically: copying a project without its linked media would break references. Cross-device project portability, linked-media collection/relinking, persistent external-folder bookmarks and launching a project directly from Files still need dedicated validation or implementation. Keeping a local media copy is not proof of complete project portability.

## Input and layout

Single-touch input drives the original pointer interaction. Two-finger viewport pan cancels the primary pointer before taking ownership. Paste and hardware navigation keys are bridged explicitly. The initial orientation request favors landscape on both device families; portrait remains supported, and the operating system may reject a geometry request. The desktop panel layout is preserved on a minimum logical canvas, with viewport zoom and pan rather than a separate mobile redesign.

## Signing and evidence

The bundle identifier is `works.valliant.unwrittenstudio`. Installation on a physical iPhone or iPad requires appropriate signing and provisioning; no signing material belongs in the repository.

See [build and status](BUILD_AND_STATUS.md) for the current validation evidence and remaining release gates. Linux rendering tests validate the original host and workflows on Linux, not UIKit integration or physical iOS behavior. Simulator frames validate startup and presentation, not every editing operation or sustained performance.
