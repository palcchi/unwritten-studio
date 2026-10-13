#!/usr/bin/env bash
# Build a device-architecture iPadOS app and package an unsigned IPA for later signing.
# Requires macOS, Xcode, Rust, and network access for Cargo dependencies.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IOS_DIR="$ROOT/ios"
OUT_DIR="${OUTPUT_DIR:-$ROOT/build/ios/unsigned}"
DERIVED_DATA="$ROOT/build/ios/DerivedData-unsigned"

if [[ "$(uname -s)" != "Darwin" ]] || ! command -v xcodebuild >/dev/null 2>&1; then
  echo "error: this build requires macOS and Xcode; it cannot run on Ubuntu/Linux." >&2
  exit 2
fi

for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  rustup target add "$target"
done

xcodebuild \
  -project "$IOS_DIR/UnwrittenStudio.xcodeproj" \
  -scheme UnwrittenStudio \
  -configuration Release \
  -destination 'generic/platform=iOS' \
  -derivedDataPath "$DERIVED_DATA" \
  CODE_SIGNING_ALLOWED=NO \
  build

APP_PATH="$DERIVED_DATA/Build/Products/Release-iphoneos/UnwrittenStudio.app"
if [[ ! -d "$APP_PATH" ]]; then
  echo "error: Xcode build succeeded but app bundle was not found at $APP_PATH" >&2
  exit 3
fi

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/Payload"
ditto "$APP_PATH" "$OUT_DIR/Payload/UnwrittenStudio.app"
IPA="$OUT_DIR/UnwrittenStudio-unsigned.ipa"
(
  cd "$OUT_DIR"
  /usr/bin/zip -qry "$IPA" Payload
)
/usr/bin/unzip -t "$IPA" >/dev/null
/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$OUT_DIR/Payload/UnwrittenStudio.app/Info.plist"
echo "Created unsigned device IPA: $IPA"
echo "This IPA is not installable until it is signed and provisioned with a compatible Apple signing identity/profile."
