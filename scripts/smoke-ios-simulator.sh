#!/usr/bin/env bash
# Build and launch the real native app on an available iPhone and iPad simulator.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/build/ios/simulator-smoke"
DERIVED="$ROOT/build/ios/DerivedData-simulator"
BUNDLE=works.valliant.unwrittenstudio
if [[ "$(uname -s)" != Darwin ]] || ! command -v xcrun >/dev/null; then
  echo 'error: simulator smoke test requires macOS and Xcode' >&2
  exit 2
fi
mkdir -p "$OUT"
xcodebuild -project "$ROOT/ios/UnwrittenStudio.xcodeproj" \
  -scheme UnwrittenStudio -configuration Release -sdk iphonesimulator \
  -destination 'generic/platform=iOS Simulator' -derivedDataPath "$DERIVED" \
  ARCHS=arm64 ONLY_ACTIVE_ARCH=YES \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO build
APP="$DERIVED/Build/Products/Release-iphonesimulator/UnwrittenStudio.app"
xcrun simctl list devices available --json > "$OUT/devices.json"
python3 - "$OUT/devices.json" > "$OUT/selected.tsv" <<'PY'
import json, sys
from pathlib import Path
all_devices = json.loads(Path(sys.argv[1]).read_text())["devices"]
devices = [d for runtime, ds in all_devices.items() if "iOS" in runtime for d in ds if d.get("isAvailable")]
for family in ("iPhone", "iPad"):
    match = next((d for d in devices if d["name"].startswith(family)), None)
    if match is None:
        raise SystemExit(f"No available {family} simulator runtime")
    print(f'{family}\t{match["udid"]}')
PY
while IFS=$'\t' read -r family device; do
  echo "Launching Unwritten Studio on $family ($device)"
  # A runner can already have a device booted; do not boot it twice.
  state="$(xcrun simctl list devices booted --json)"
  if ! grep -q "$device" <<< "$state"; then xcrun simctl boot "$device"; fi
  xcrun simctl bootstatus "$device" -b
  xcrun simctl install "$device" "$APP"
  xcrun simctl launch --terminate-running-process "$device" "$BUNDLE" --ui-smoke | tee "$OUT/$family-launch.txt"
  pid="$(awk -F': ' 'NF>1 {print $NF}' "$OUT/$family-launch.txt" | tail -1)"
  [[ "$pid" =~ ^[0-9]+$ ]] || { echo "error: launch returned no process ID" >&2; exit 3; }
  # Allow the Rust engine to initialize and render, then ensure it did not immediately crash.
  sleep 15
  kill -0 "$pid"
  ps -p "$pid" -o pid=,comm= > "$OUT/$family-process.txt"
  xcrun simctl io "$device" screenshot "$OUT/$family-workspace.png"
  xcrun simctl terminate "$device" "$BUNDLE"
  xcrun simctl shutdown "$device"
done < "$OUT/selected.tsv"
echo "Simulator launch evidence saved in $OUT (not a physical-device functional certification)."
