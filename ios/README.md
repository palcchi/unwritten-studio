# iPadOS application

This directory contains the native SwiftUI application target. It links the local Rust static library built by `scripts/build-ios.sh`; there is no web server, WebView, or remote renderer in the iPad app.

Read [the implementation status and Mac build guide](../docs/ios/BUILD_AND_STATUS.md) for what is implemented, what's unverified, signing requirements, and physical-device test steps.

The C ABI is declared in `Rust/include/unwritten_studio_core.h`. Keep the ownership rules in sync with `apps/unwritten-studio-core/src/lib.rs` if the boundary changes. The Rust side owns the project/render engine; Swift owns iPad navigation, file/document access, app sandbox paths, and view state.
