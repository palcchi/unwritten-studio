# Unwritten Studio on iOS

## The editor now comes from the original frontend

The iOS app's root is `OriginalWorkspaceView`. It hosts the existing Rust `unwritten-studio-ui-egui` frontend; UIKit does not recreate its panels or menus. The original frontend continues to own its dock tree, timeline, property controls, keyframe tools, effects panels, dialogs, menu hierarchy, command dispatch and session. The branding remains Unwritten Studio.

`UnwrittenStudioApp::embedded_logic` and `embedded_ui` share the same implementation used by the desktop `eframe::App` integration. This is a host integration, not an independent look-alike editor. `StudioModel.swift` and `WorkspaceView.swift` remain as historical source, but are no longer part of the app's compiled Sources phase or its root scene.

## What iOS supplies

The C ABI in `ios/Rust/include/unwritten_studio_core.h` passes viewport dimensions, pointer and keyboard events, and native file-dialog results to Rust. The Rust host renders the original egui UI through wgpu, returns owned RGBA pixels, and reports keyboard, clipboard, URL and file-dialog requests. UIKit displays those pixels and supplies native document pickers and text input.

This host uses GPU readback and a copied image for presentation. It is **not** a zero-copy CAMetalLayer integration, and 30 Hz scheduling is not a performance guarantee. The existing original rendering and codec pipeline is reused; AVFoundation decoding or hardware encoding has not been added.

The target supports iPhone and iPad on iOS 17 or later. It preserves the original desktop UI rather than replacing it with a different compact editor. On small screens a minimum 1024×640 logical canvas is fitted without reflowing the panels; pinch zoom and two-finger viewport pan preserve the original layout. The initial scene requests landscape on both families while keeping portrait supported. Physical-device pointer and keyboard usability still needs verification.

Native preview audio now has a dedicated output bridge: the original frontend's `AudioDevice` factory shares its existing mixer feed with an independent callback handle, and `AVAudioSourceNode` consumes stereo Float32 at 48 kHz. Its callback never accesses the thread-confined editor; planar conversion uses a bounded stack buffer. The worker starts/stops the engine with its presentation lifecycle and releases the audio handle only after stopping callbacks. Successful engine startup alone does not prove audible output, interruption recovery, latency compensation or A/V sync on a physical device.

## Validation must match this new root

A successful IPA from the earlier SwiftUI shell does not validate this host. The new source must separately pass Rust checks, Xcode device linking and simulator launch. The workflow builds an unsigned device IPA and runs `scripts/smoke-ios-simulator.sh` on iPhone and iPad. Simulator builds explicitly use arm64, matching the Rust simulator library.

`--ui-smoke` is a CI-only launch argument for opening an original demo workspace. The smoke test requires ten successfully displayed, non-black editor frames, a running native audio engine and a JSON readiness record, and fails on a startup/render/audio initialization error. Screenshots, readiness/error records and launch evidence are uploaded as artifacts. A live process alone is not accepted. This still does not certify audible output or every editing workflow.

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

The latest Linux core test run passes all 12 tests, including the two explicitly enabled GPU-backed original-host tests. The workflow test creates the actual `NativeUi`/`UnwrittenStudioApp`, forwards text/paste/key input, completes the asynchronous dialog callback, edits and undoes/redoes a real session, saves/reopens a project, imports a generated SVG and exports Lottie JSON. Some assertions deliberately use session commands; this is not a claim that UIKit touch/file-picker interactions were exercised. The callback contention regression also passes, proving it returns silence without advancing the audio clock while the mixer queue is locked.

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
