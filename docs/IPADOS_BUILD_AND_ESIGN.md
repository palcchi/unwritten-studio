# Native iPadOS build and eSign handoff

Unwritten Studio is built as a native SwiftUI/UIKit application with a Rust static-library core. The iPadOS target does not embed the web app or use a WebView.

## Build an unsigned device IPA in GitHub Actions

The workflow `.github/workflows/ios-compatibility.yml` runs on a macOS GitHub Actions runner. It:

1. Checks the Rust core for `aarch64-apple-ios` and `aarch64-apple-ios-sim`.
2. Builds the Xcode `UnwrittenStudio` target for a generic physical iOS device with code signing disabled.
3. Packages the device app bundle into `UnwrittenStudio-unsigned.ipa`.
4. Uploads that file as the `unwritten-studio-unsigned-ipa` workflow artifact.

Open the repository's **Actions** tab, select **iPadOS Native Build**, and open the latest successful run. Download the artifact named `unwritten-studio-unsigned-ipa` and extract the ZIP artifact to get the IPA file.

A successful workflow is required before treating the artifact as built. If the workflow fails, inspect its job logs rather than attempting to sign a broken package.

## Sign later with eSign

The CI output is intentionally **unsigned**. It is a device build, not a simulator build, but it is not installable until signed.

In eSign, use the IPA-signing/import workflow and provide a compatible signing certificate/private key (commonly a `.p12`) and provisioning profile (commonly a `.mobileprovision`) when your signing method requires them. Use the same App ID / bundle identifier as the app's `Info.plist` and provisioning profile. Do not rename the bundle identifier after obtaining the profile. Follow eSign's current UI labels; they can vary by version.

Important distinctions:

- **Unsigned IPA:** app bundle packaged for a physical device; not installable yet.
- **Development/ad-hoc signed IPA:** requires a compatible certificate and provisioning profile; ad-hoc profiles also need the target device's UDID.
- **Enterprise-signed IPA:** only for authorized enterprise distribution and subject to Apple's rules.
- **App Store/TestFlight:** requires App Store Connect distribution signing and the appropriate upload/export process; it is not the same as ordinary eSign sideloading.

A certificate/profile mismatch, expired/revoked certificate, missing device registration, or unsupported entitlements can make signing or installation fail. Never share a private signing key or its password in the repository or GitHub Actions logs.

## Local unsigned build on a Mac

Requirements: macOS, Xcode with iOS SDK, Rust 1.95+ and internet access for Cargo dependencies.

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
./scripts/build-unsigned-ipa.sh
```

Output: `build/ios/unsigned/UnwrittenStudio-unsigned.ipa`.

For a directly development-signed archive, use `scripts/build-ios.sh` and set `DEVELOPMENT_TEAM` to the Apple Developer Team ID configured in Xcode. That path uses automatic provisioning and exports a development-signed IPA; it is separate from the unsigned eSign handoff.

## Current feature and verification boundaries

The current native vertical slice supports basic project creation/open/save, listing project items and composition layers, adding basic layers, selecting a timeline layer, playhead scrubbing, undo/redo, media import into app-owned storage, and a local RGBA preview. The Rust renderer attempts a headless `wgpu` adapter using Metal on iOS and falls back to CPU if initialization fails.

A green Rust target check does not prove Xcode linking, launch on a physical iPad, GPU runtime correctness, or interactive performance. The CI workflow's build result and logs must be checked. Advanced effects editing, keyframes, render-queue export, audio playback, full property editing, custom canvas gestures, Pencil-specific tools, and device performance profiling remain incomplete or unverified.

Do not describe this project as a complete After Effects replacement or claim Metal is always active. Test the signed IPA on a physical iPad and record the device model, iPadOS version, renderer selected, preview latency, memory use, and thermal behavior.
