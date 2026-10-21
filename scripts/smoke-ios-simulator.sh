#!/usr/bin/env bash
# Verify actual original-editor frames in BOTH orientations, not merely a live process.
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
failed=0
while IFS=$'\t' read -r family device; do
  echo "Booting $family ($device)"
  state="$(xcrun simctl list devices booted --json)"
  if ! grep -q "$device" <<< "$state"; then xcrun simctl boot "$device"; fi
  xcrun simctl bootstatus "$device" -b
  xcrun simctl install "$device" "$APP"
  container="$(xcrun simctl get_app_container "$device" "$BUNDLE" data)"
  checks="$container/Documents/UISmoke"
  mkdir -p "$checks"
  for orientation in portrait landscape; do
    label="$family-$orientation"
    echo "Launching original workspace: $label"
    rm -f "$checks/workspace-ready.json" "$checks/workspace-error.json"
    xcrun simctl launch --terminate-running-process "$device" "$BUNDLE" --ui-smoke "--smoke-$orientation" | tee "$OUT/$label-launch.txt"
    pid="$(awk -F': ' 'NF>1 {print $NF}' "$OUT/$label-launch.txt" | tail -1)"
    [[ "$pid" =~ ^[0-9]+$ ]] || { echo 'error: launch returned no process ID' >&2; exit 3; }
    ready=0
    for second in $(seq 1 120); do
      if [[ -f "$checks/workspace-error.json" ]]; then
        cp "$checks/workspace-error.json" "$OUT/$label-error.json"
        cat "$OUT/$label-error.json"
        break
      fi
      if ! kill -0 "$pid" 2>/dev/null; then
        echo "error: $label editor process crashed before rendering" >&2
        break
      fi
      if [[ -f "$checks/workspace-ready.json" ]]; then
        cp "$checks/workspace-ready.json" "$OUT/$label-ready.json"
        ready=1
        break
      fi
      sleep 1
    done
    xcrun simctl io "$device" screenshot "$OUT/$label-workspace.png"
    if [[ "$ready" == 1 ]]; then
      ps -p "$pid" -o pid=,comm= > "$OUT/$label-process.txt"
      if ! python3 - "$OUT/$label-ready.json" "$orientation" <<'PY'
import json, math, sys
status = json.load(open(sys.argv[1]))
assert status.get("ok") is True and status.get("nonBlack") is True, status
assert status.get("renderedFrames", 0) >= 10 and status.get("width", 0) > 0 and status.get("height", 0) > 0, status
assert status.get("audioEngineRunning") is True, status
vw, vh = status.get("viewportWidth", 0), status.get("viewportHeight", 0)
cw, ch = status.get("canvasWidth", 0), status.get("canvasHeight", 0)
fit = status.get("fitZoom", 0)
assert all(math.isfinite(x) and x > 0 for x in (vw, vh, cw, ch, fit)), status
assert (vh > vw) == (sys.argv[2] == "portrait"), status
assert abs(cw * fit - vw) <= 2 and abs(ch * fit - vh) <= 2, status
assert abs(status["width"] / status["height"] - cw / ch) < 0.01, status
print("Verified edge-filling original workspace:", status)
PY
      then failed=1; fi
    else
      echo "error: $label did not display ten valid original-editor frames" >&2
      failed=1
    fi
    xcrun simctl terminate "$device" "$BUNDLE" || true
  done
  xcrun simctl shutdown "$device"
done < "$OUT/selected.tsv"
[[ "$failed" == 0 ]] || exit 4
echo "Original editor filled portrait and landscape viewports with native audio engine running on iPhone and iPad. Evidence: $OUT. This does not certify audible output, audio sync, every editing workflow or physical devices."
