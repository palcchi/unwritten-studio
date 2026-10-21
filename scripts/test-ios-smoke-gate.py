#!/usr/bin/env python3
"""Exercise the exact shell-embedded acceptance validator with invalid evidence."""
import contextlib
import copy
import io
import json
from pathlib import Path
import sys
import tempfile

root = Path(__file__).resolve().parents[1]
script = (root / "scripts/smoke-ios-simulator.sh").read_text()
# UI tests can reinstall the application; evidence must use the post-test UUID.
launch = script.index("if xcodebuild test-without-building")
refresh = script.index('container="$(xcrun simctl get_app_container', launch)
readiness = script.index('if [[ -f "$checks/workspace-ready.json"', launch)
assert launch < refresh < readiness

marker = '"$container" <<\'PY\'\n'
validator = compile(script.split(marker, 1)[1].split('\nPY\n', 1)[0], "smoke-validator", "exec")

with tempfile.TemporaryDirectory() as temporary:
    container = Path(temporary)
    recovery = container / "Library/Application Support/Unwritten Studio/embedded-session-recovery.json"
    recovery.parent.mkdir(parents=True)
    status_file = container / "status.json"
    checkpoint = {"app": {}, "items": [{"name": "Smoke project"}]}
    base = dict(ok=True, nonBlack=True, renderedFrames=10, audioEngineRunning=True,
                width=1024, height=1366, viewportWidth=1024, viewportHeight=1366,
                canvasWidth=1024, canvasHeight=1366, fitZoom=1)

    def accepts(status, orientation="portrait", recovery_data=checkpoint):
        status_file.write_text(json.dumps(status))
        if recovery_data is None:
            recovery.unlink(missing_ok=True)
        else:
            recovery.write_text(json.dumps(recovery_data))
        previous = sys.argv
        sys.argv = ["validator", str(status_file), orientation, str(container)]
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                exec(validator, {})
            return True
        except AssertionError:
            return False
        finally:
            sys.argv = previous

    assert accepts(base)
    landscape = copy.deepcopy(base)
    for a, b in [("width", "height"), ("viewportWidth", "viewportHeight"), ("canvasWidth", "canvasHeight")]:
        landscape[a], landscape[b] = landscape[b], landscape[a]
    assert accepts(landscape, "landscape")
    rejected = 0
    for field, value in [("ok", False), ("nonBlack", False), ("renderedFrames", 9),
                         ("audioEngineRunning", False), ("width", 0), ("viewportWidth", 0),
                         ("fitZoom", float("nan")), ("fitZoom", 0.9), ("width", 100)]:
        invalid = dict(base, **{field: value})
        assert not accepts(invalid), (field, value)
        rejected += 1
    assert not accepts(base, "landscape"), "Portrait evidence must never pass landscape"
    square = dict(base, width=1024, height=1024, viewportHeight=1024, canvasHeight=1024)
    assert not accepts(square, "landscape"), "A square window is not landscape"
    assert not accepts(base, recovery_data=None)
    assert not accepts(base, recovery_data={"app": {}, "items": []})
    print(f"ok: portrait/landscape accepted; {rejected + 4} invalid smoke/recovery cases rejected")
