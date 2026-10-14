# Unwritten Studio for iPadOS — build and status

## Implemented in this vertical slice

- A **SwiftUI/UIKit iPadOS app target** (`ios/UnwrittenStudio.xcodeproj`) with an adaptive editorial workspace, in-project item browser, composition preview, timeline rows, inspector, toolbar, native project-document import/export, and keyboard shortcuts.
- A Rust static-library bridge around the existing Unwritten Studio `Session`, command registry, project model, media importer, serialization, and renderer. The app does not load a webpage or use a `WKWebView`.
- Project creation, `.uwproj`/`.uwprojx` open and `.uwproj` save, composition/layer listing, adding solid/shape/text layers, selecting a timeline layer, undo/redo, playhead scrubbing, image/video/audio import into app-owned storage and the active composition, and locally rendered RGBA preview.
- The iOS Rust session attempts `unwritten-studio-gpu::Gpu::headless()`; wgpu can use the Apple Metal backend on iOS. If adapter/device initialization fails, the existing CPU renderer remains the fallback. The current rendering path can still perform substantial source/effect work on CPU, and no iPad benchmark or thermal/memory profile has been run.
- First iPad usability pass: increased small workspace typography, added Dynamic Type scaling, enlarged primary touch targets to at least 44 pt, and made portrait inspector/timeline heights respond to available view geometry; the inspector now scrolls when space is constrained. This changes layout only and does not claim that the broader editor feature set is complete.
- Corrected adaptive layout selection to require a genuinely landscape-shaped viewport (at least 1.15:1), not just a large width. This keeps large iPads in portrait in the stacked canvas/inspector/timeline layout instead of squeezing the canvas into the three-column landscape layout.
- The selected-layer Inspector now reads position, scale, rotation, and opacity from the Rust property API and applies plus/minus edits via `prop.set`; successful edits refresh the local preview and use engine history, with opacity bounded to 0–100.

## Not yet implemented / unverified

- No signed or unsigned `.ipa` has been produced in this Ubuntu environment. This machine has no `xcodebuild`, Apple SDK, iOS simulator runtime, Apple signing identity, provisioning profile, or physical iPad connection. An installable IPA is **not** being claimed.
- `cargo check --locked -p unwritten-studio-core` passes for both `aarch64-apple-ios` and `aarch64-apple-ios-sim`; this checks Rust's Apple-target code and Metal backend dependencies, but does **not** link against an Apple SDK or produce an app. Xcode compilation/archive, device launch, native document picker behavior, touch/Pencil/pointer behavior, rotation/split-screen/Stage Manager, and Metal runtime compatibility remain unverified.
- The native screen is a focused first vertical slice, not a port of the full Unwritten Studio desktop UI. The current UI exposes basic project/layer workflows; advanced effects, keyframe editing, audio playback, render-queue export, property editing beyond the basic transform steppers, drag/reorder, Apple Pencil drawing, and advanced import options remain follow-up work.
- The project browser lists the current document's items; recent-project history and visual thumbnails are not implemented. Native controls and scrolling/slider touch work are present, but custom canvas gestures, Pencil tools and hardware pointer behavior have not been tested.
- GPU initialization is attempted, not guaranteed. Unsupported formats, shader/device limits, allocations, or GPU-specific effects can fall back to CPU. Native execution is not presumed faster. Performance must be measured on supported iPads before setting product claims or default preview quality.

## Requirements

- macOS with the Xcode version required by the installed Swift/iOS SDK, command line tools, and Rust 1.95+ with targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`.
- A valid Apple Development signing certificate and provisioning profile associated with a team for installation on registered iPads. For App Store/TestFlight distribution, App Store Connect distribution signing and corresponding export configuration are needed instead.
- Rust dependency sources must be fetched by Cargo. No credentials or certificates are checked into this repository.

## Build on a compatible Mac

From the repository root:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
DEVELOPMENT_TEAM=YOUR10CHARTEAMID ./scripts/build-ios.sh
```

The script compiles the device Rust static library, archives the Xcode app for generic iOS devices, exports a development-signed IPA, and verifies that the IPA exists. Configure the team in Xcode or permit automatic provisioning with an Apple account already configured in Xcode. To use the Xcode UI instead, first build `unwritten-studio-core` for `aarch64-apple-ios`, then open `ios/UnwrittenStudio.xcodeproj`, select the UnwrittenStudio target, choose a Team under Signing & Capabilities, and archive for a generic iOS device.

`EXPORT_METHOD` defaults to `development`; valid values follow `xcodebuild -exportArchive` (`development`, `ad-hoc`, or `app-store-connect`). Ad Hoc distribution still requires a distribution identity and registered device UDIDs. App Store Connect distribution is not directly sideloadable.

## Install and test on iPad

1. Connect a development-signed IPA through Xcode Devices and Simulators, Apple Configurator, or an approved MDM distribution path. The iPad must be included in the development provisioning profile.
2. Open a project or create one, add a solid/text layer, scrub the timeline, and save/export an `.uwproj` document.
3. Import an image/video/audio file and verify that the copied media remains linked after closing/reopening the project.
4. Verify render correctness against the same frame from the desktop/CLI, compare CPU and Metal output, and capture GPU adapter, preview render time, memory, and thermal behavior on each target device class.
5. Verify portrait/landscape, split view, Stage Manager, touch targets, an external keyboard, pointer, and Pencil on supported hardware.

## Architecture and migration notes

- Reused portable Rust layers: `unwritten-studio-project`, `unwritten-studio-engine`, `unwritten-studio-render`, `unwritten-studio-media`, and the `unwritten-studio-host` session assembly; `unwritten-studio-gpu` is used opportunistically for a headless wgpu compositor.
- The existing native frontend is eframe/egui/winit and has desktop-only window/menu/audio/file-dialog assumptions; it is **not** reused as an iPad shell. The web crate is also not part of this native target.
- The bridge is a small C ABI static library, with pointer ownership documented in `ios/Rust/include/unwritten_studio_core.h`. Unsafe pointer conversion/release code is isolated to the ABI module. Swift owns native document URLs and copies imported media into app-owned support storage before passing stable paths to Rust.
- No AVFoundation media decoding or hardware encoder integration has been added. The current media pipeline remains the existing Rust codec path and therefore must be profiled on iPad.

## Upstream license

Unwritten Studio is licensed under MIT OR Apache-2.0. This derivative retains the upstream `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, and attribution files. The Unwritten Studio app code remains under the repository's MIT OR Apache-2.0 license unless subsequently separated.
