#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IOS_DIR="$ROOT/ios"
BUILD_DIR="$ROOT/build/ios"
TEAM_ID="${DEVELOPMENT_TEAM:-}"
EXPORT_METHOD="${EXPORT_METHOD:-development}"

if [[ "$(uname -s)" != "Darwin" ]] || ! command -v xcodebuild >/dev/null 2>&1; then
  echo "error: iPadOS archive/signing requires macOS with Xcode; this host is $(uname -s)." >&2
  exit 2
fi
if [[ -z "$TEAM_ID" ]]; then
  echo "error: set DEVELOPMENT_TEAM to the Apple Developer Team ID configured for signing." >&2
  exit 2
fi
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  rustup target add "$target"
done
mkdir -p "$BUILD_DIR/iphoneos" "$BUILD_DIR/archive" "$BUILD_DIR/export"

(cd "$ROOT" && cargo build --locked --release -p unwritten-studio-core --target aarch64-apple-ios)
cp "$ROOT/target/aarch64-apple-ios/release/libunwritten_studio_core.a" "$BUILD_DIR/iphoneos/libunwritten_studio_core.a"

xcodebuild \
  -project "$IOS_DIR/UnwrittenStudio.xcodeproj" \
  -scheme UnwrittenStudio \
  -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath "$BUILD_DIR/archive/UnwrittenStudio.xcarchive" \
  DEVELOPMENT_TEAM="$TEAM_ID" \
  CODE_SIGN_STYLE=Automatic \
  -allowProvisioningUpdates \
  archive

cat > "$BUILD_DIR/ExportOptions.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>${EXPORT_METHOD}</string>
  <key>teamID</key><string>${TEAM_ID}</string>
  <key>signingStyle</key><string>automatic</string>
  <key>stripSwiftSymbols</key><true/>
  <key>compileBitcode</key><false/>
</dict></plist>
PLIST

xcodebuild \
  -exportArchive \
  -archivePath "$BUILD_DIR/archive/UnwrittenStudio.xcarchive" \
  -exportPath "$BUILD_DIR/export" \
  -exportOptionsPlist "$BUILD_DIR/ExportOptions.plist" \
  DEVELOPMENT_TEAM="$TEAM_ID" \
  -allowProvisioningUpdates

IPA="$BUILD_DIR/export/UnwrittenStudio.ipa"
if [[ ! -s "$IPA" ]]; then
  echo "error: Xcode export finished without an IPA at $IPA" >&2
  exit 3
fi
/usr/bin/unzip -t "$IPA" >/dev/null
echo "Verified exported IPA: $IPA"
