# Unwritten Studio on iOS

## The editor now comes from the original frontend

The iOS app's root is `OriginalWorkspaceView`. It hosts the existing Rust `unwritten-studio-ui-egui` frontend; UIKit does not recreate its panels or menus. The original frontend continues to own its dock tree, timeline, property controls, keyframe tools, effects panels, dialogs, menu hierarchy, command dispatch and session. The branding remains Unwritten Studio.

`UnwrittenStudioApp::embedded_logic` and `embedded_ui` share the same implementation used by the desktop `eframe::App` integration. This is a host integration, not an independent look-alike editor. `StudioModel.swift` and `WorkspaceView.swift` remain as historical source, but are no longer part of the app's compiled Sources phase or its root scene.

## What iOS supplies

The C ABI in `ios/Rust/include/unwritten_studio_core.h` passes viewport dimensions, pointer and keyboard events, and native file-dialog results to Rust. The Rust host renders the original egui UI through wgpu, returns owned RGBA pixels, and reports keyboard, clipboard, URL and file-dialog requests. UIKit displays those pixels and supplies native document pickers and text input.

This host uses GPU readback and a copied image for presentation. It is **not** a zero-copy CAMetalLayer integration, and 30 Hz scheduling is not a performance guarantee. The existing original rendering and codec pipeline is reused; AVFoundation decoding or hardware encoding has not been added.

The target supports iPhone and iPad on iOS 17 or later. It preserves the original desktop UI rather than replacing it with a different compact editor. The workspace is fitted to the actual safe-area viewport with a minimum 1024-point logical width while retaining the viewport aspect ratio, so portrait and landscape use the available screen without a fixed 1024×640 letterbox. Pinch zoom and two-finger viewport pan preserve the original layout; rotation and safe-area changes re-fit the canvas while retaining the user's zoom/center where possible. The initial scene requests landscape on both families while keeping portrait supported. Physical-device pointer and keyboard usability still needs verification.

Panel dividers have an always-visible grip and a larger touch hit area. Drag a divider with one finger to change pane width or height; two fingers pan the overall workspace. Layout and drag use the same responsive minimum size. UIKit does not delay single-finger touches before delivering them to the editor.

Import Footage offers **Files** or **Photos**. Files supports the engine's existing media types, with coordinated reads for iCloud/document providers on a background queue. Photos uses the system picker without requesting whole-library access. Selected files are copied to persistent app-owned storage before the engine receives their paths. HEIC/HEIF images are converted to JPEG or PNG (alpha preserved), with explicit size limits; other formats retain their original bytes, including layered PSD, animated GIF and high-bit-depth TIFF. Actual provider/picker interactions and individual codecs still need physical-device testing.

Automatic render-queue outputs on iOS go to **On My iPad/iPhone → Unwritten Studio → Exports** (`Documents/Exports`) rather than the container root or a possibly read-only external project's parent. Explicit output paths and default-folder choices remain honored: an old queue item pointing outside the sandbox may still need a new **Output To** choice. File-write errors include the failing destination. This fixes the default-path permission policy, not a GPU or M2 hardware limitation.

New project saves default to **`.unw`**, the existing versioned native JSON format. Open/import and Save As recognize it as a project; legacy `.uwproj`, `.uwprojx`, `.ecproj` and `.ecprojx` remain readable. iOS registers `.unw` as an app document type. Binary After Effects **`.aep` now has experimental native read-only import**, through a bounded Rust reader and adapter inside the app. Choose it from the in-app Files picker and confirm opening an experimental copy; no After Effects desktop or Python is required for this import. The imported project opens untitled/dirty, preserves its AEP source and reports unsupported content and missing media. See [native AEP scope](../aep-native-import.md). Direct `.aep` writing and `.aepx` reading/writing remain unsupported; native JSON is never written under those extensions. Unwritten's native XML is not Adobe AEPX. The optional [desktop AE bridge](../file-format-parity.md#after-effects-desktop-bridge-experimental-partial) remains available for `.unwae` interchange and JSX export to AE desktop. Neither route is a full-fidelity round trip.

Save/export dialogs request a destination folder through Files and confirm replacing an existing file. Render outputs are checked again before rendering. Native session recovery checkpoints project and saved-view state locally, including a lifecycle checkpoint when presentation stops; recovery opens untitled to avoid overwriting the source. Corrupt recovery data is retained and reported rather than replaced. These paths have synthetic tests, not physical-device Files-provider or force-quit certification.

Home and the application menus no longer advertise bundled demo projects, template galleries or guided tutorials. Application source, runtime infrastructure, user-created template support and internal test/smoke generators are retained. UI actions that replace a dirty project ask **Save / Don't Save / Cancel**. For an untitled project on iOS, Save waits for the native Files picker and a successful write before continuing the requested switch. Cancellation, failed writes, stale callbacks and edits made while the picker is open do not switch projects; each save continuation is one-shot.

Native preview audio now has a dedicated output bridge: the original frontend's `AudioDevice` factory shares its existing mixer feed with an independent callback handle, and `AVAudioSourceNode` consumes stereo Float32 at 48 kHz. Its callback never accesses the thread-confined editor; planar conversion uses a bounded stack buffer. The worker starts/stops the engine with its presentation lifecycle and releases the audio handle only after stopping callbacks. Successful engine startup alone does not prove audible output, interruption recovery, latency compensation or A/V sync on a physical device.

## Validation must match this new root

A successful IPA from the earlier SwiftUI shell does not validate this host. The new source must separately pass Rust checks, Xcode device linking and simulator launch. The workflow builds an unsigned device IPA and runs `scripts/smoke-ios-simulator.sh` on iPhone and iPad. Simulator builds explicitly use arm64, matching the Rust simulator library.

`--ui-smoke` is a CI-only launch argument for opening an original demo workspace. On iOS it first renders a separate tiny H.264 MP4 to the actual default sandbox output folder and reimports it, failing on any permission, encoding or import error. The smoke test then requires ten successfully displayed, non-black editor frames, a running native audio engine and a JSON readiness record, and fails on a startup/render/audio initialization error. Screenshots, readiness/error records and launch evidence are uploaded as artifacts. A live process alone is not accepted. This still does not certify Photos/Files picker interaction, audible output or every editing workflow.

Before calling this a complete iOS port, verify the following on signed physical devices:

- Real document creation, editing, reopening and persistence of linked media, including security-scoped files and relative paths.
- Dock resizing, timeline dragging, keyframe editing, property entry, effects, undo/redo and keyboard shortcuts.
- Import formats, render-queue completion and the correctness of exported files.
- Audio output, app suspension/resume, recovery, clipboard, Pencil, pointer and external keyboard behavior.
- Metal compatibility, memory, responsiveness, sustained playback and thermal behavior.

The original UI being reused does not by itself make all desktop OS integrations available on iOS. Persistent external-folder access, direct Files-app project launch, portable project/media collection and native audio interruption/route handling remain open integration work. No claim of 100% feature parity or physical-device certification is made.

## Rendering checks

The Linux/Vulkan presenter has rendered the real original demo workspace, and the core geometry/session/render tests and frontend lifecycle test pass. An initial simulator test exposed a genuine Metal limit mismatch: wgpu desktop defaults request 16 inter-stage shader variables, while iOS Metal reports 15. The presenter now requests portable downlevel limits with the adapter's real texture dimensions. A regression test covers that cap. The app retains the original egui-wgpu renderer; no software look-alike UI is substituted.

The initial process-only simulator run was not valid render evidence and its screenshots showed an initialization alert. Only the stricter frame-verifying run is relevant for release readiness.

The 21 October Linux regression run passes 632 engine tests, 15 core tests and 14 shared-docking integration tests, with two engine and two GPU-backed core tests left ignored by those default runs. Eight viewport geometry cases plus rotation/safe-area source-contract checks pass. Bridge tests use a synthetic AE scripting DOM, not a real After Effects installation. Earlier GPU-backed workflow coverage creates the actual `NativeUi`/`UnwrittenStudioApp`, forwards text/paste/key input, completes the asynchronous dialog callback, edits and undoes/redoes a real session, saves/reopens a project, imports a generated SVG and exports Lottie JSON. Some assertions deliberately use session commands; this is not a claim that UIKit touch/file-picker interactions were exercised. No new IPA, simulator success or physical-device validation is claimed for this patch until its GitHub iOS workflow completes.

The native-AEP follow-up passes 635 engine tests (2 ignored), 15 core tests (2 ignored), 9 unsaved-project guard tests, 13 parser/adapter tests, and the Home control regression. Targeted Clippy with warnings denied, assets and layering pass. A real private AEP opens as 4 compositions / 19 supported layers, survives native serialization and import/undo, and keeps source bytes unchanged; 184 detected losses and 5 missing-media paths make this explicitly a partial import. A Linux headless Home snapshot was inspected; this is not an iOS-device screenshot or a new IPA.

The repository-wide `cargo xtask ci` remains blocked by existing import-order formatting differences outside this patch on the available toolchain. Targeted tests and the iOS workflow do not stand in for a green repository-wide gate.

## Build and signing

On a Mac with Xcode and Rust 1.95 or later:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
./scripts/build-unsigned-ipa.sh
```

The unsigned IPA is written to `build/ios/unsigned/UnwrittenStudio-unsigned.ipa`. It must be signed before installation. No signing credentials or provisioning profiles are included.

For a development-signed archive, configure the Apple team and signing identity in Xcode, or use:

```sh
DEVELOPMENT_TEAM=YOUR10CHARTEAMID ./scripts/build-ios.sh
```

## License

The derivative retains the repository's MIT OR Apache-2.0 license, upstream notices and attribution files. Existing bundled font licenses are retained. Rebranding does not remove upstream license obligations.
