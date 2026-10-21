#!/usr/bin/env python3
"""Regression checks for OriginalWorkspaceView's aspect-matched canvas contract.

This mirrors the small pure calculation in OriginalWorkspaceView.swift so it can
run on Linux without Xcode or a simulator. The simulator smoke test remains the
proof of actual UIKit layout and Rust rendering.
"""
from __future__ import annotations

import math
import pathlib
import plistlib

MIN_LOGICAL_WIDTH = 1024.0
MAX_RENDER_SIDE = 4096.0


def canvas_size(width: float, height: float) -> tuple[float, float]:
    assert math.isfinite(width) and math.isfinite(height) and width > 0 and height > 0
    logical_width = max(MIN_LOGICAL_WIDTH, width)
    logical_height = logical_width * height / width
    factor = min(1.0, MAX_RENDER_SIDE / max(logical_width, logical_height))
    return logical_width * factor, logical_height * factor


def check(width: float, height: float, scale: float) -> None:
    cw, ch = canvas_size(width, height)
    fit = min(width / cw, height / ch, 1.0)
    assert all(math.isfinite(v) and v > 0 for v in (cw, ch, fit))
    assert max(cw, ch) <= MAX_RENDER_SIDE + 1e-9
    assert abs(cw / ch - width / height) < 1e-9
    # The fitted canvas reaches both safe viewport edges; no letterbox dead space.
    assert abs(cw * fit - width) <= 1e-9
    assert abs(ch * fit - height) <= 1e-9
    # Worker scale clamping cannot create a >4096 framebuffer dimension.
    worker_scale = min(max(scale, 0.25), min(2.0, MAX_RENDER_SIDE / max(cw, ch)))
    assert max(cw, ch) * worker_scale <= MAX_RENDER_SIDE + 1e-9


def check_host_contract() -> None:
    root = pathlib.Path(__file__).resolve().parents[1]
    source = (root / "ios/UnwrittenStudio/OriginalWorkspaceView.swift").read_text()
    # UIKit must re-fit after an orientation transition and after safe-area-only
    # changes; a bounds-size-only cache leaves stale notch/home-indicator insets.
    assert "viewWillTransition(to size: CGSize" in source
    assert "viewSafeAreaInsetsDidChange()" in source
    assert "lastViewportSafeAreaInsets" in source
    assert "rootView.safeAreaLayoutGuide" in source
    assert "contentInsetAdjustmentBehavior = .never" in source
    assert "requestGeometryUpdate" not in source, "Device rotation belongs to XCUITest, not iPad window geometry requests"
    assert 'rootView.accessibilityIdentifier = "workspace-smoke-status"' in source

    app = (root / "ios/UnwrittenStudio/UnwrittenStudioApp.swift").read_text()
    assert "@UIApplicationDelegateAdaptor(UnwrittenStudioAppDelegate.self)" in app
    assert 'return .landscapeRight' not in app, "Smoke must exercise the production orientation mask"

    driver = (root / "ios/UnwrittenStudioUITests/WorkspaceOrientationTests.swift").read_text()
    assert "XCUIDevice.shared.orientation" in driver
    assert "func testPortrait()" in driver and "func testLandscape()" in driver
    assert "screenshot.image.size.height > screenshot.image.size.width" in driver
    assert "app.frame.height > app.frame.width" in driver
    assert "XCTAssertEqual(app.state, .runningForeground" in driver
    assert 'XCTAssertEqual(status.value as? String, "ready"' in driver
    assert "attachment.lifetime = .keepAlways" in driver
    assert 'userInterfaceIdiom == .pad ? .all : .allButUpsideDown' in app

    with (root / "ios/UnwrittenStudio/Info.plist").open("rb") as handle:
        plist = plistlib.load(handle)
    assert "UIInterfaceOrientationPortrait" in plist["UISupportedInterfaceOrientations"]
    assert "UIInterfaceOrientationLandscapeLeft" in plist["UISupportedInterfaceOrientations"]
    assert "UIInterfaceOrientationLandscapeRight" in plist["UISupportedInterfaceOrientations"]
    assert len(plist["UISupportedInterfaceOrientations~ipad"]) == 4
    assert not plist.get("UIRequiresFullScreen", False), "Keep iPad multitasking available"


def main() -> None:
    # iPhone-sized safe viewports, including the tall portrait case that used to
    # exceed the native framebuffer when multiplied by a 3x Retina scale.
    cases = [
        (390, 760, 3), (393, 852, 3), (844, 343, 3),
        # iPad full and split views in both orientations.
        (1024, 694, 2), (1366, 954, 2), (694, 954, 2), (507, 694, 2),
        # An intentionally extreme ratio exercises the logical-side cap.
        (320, 3000, 3),
    ]
    for case in cases:
        check(*case)
    check_host_contract()
    print(f"ok: {len(cases)} viewport geometries plus host rotation/safe-area contract")


if __name__ == "__main__":
    main()
