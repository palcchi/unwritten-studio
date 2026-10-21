#!/usr/bin/env python3
"""Check Files handoff wiring; on macOS execute the production Swift inbox.

These tests do not replace physical iPad Files/iCloud acceptance testing.
"""
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
host = (ROOT / "ios/UnwrittenStudio/OriginalWorkspaceView.swift").read_text()
app = (ROOT / "ios/UnwrittenStudio/UnwrittenStudioApp.swift").read_text()
rust = (ROOT / "apps/unwritten-studio-core/src/native_ui.rs").read_text()
assert "@StateObject private var externalProjectInbox" in app
assert ".onOpenURL" in app and "externalProjectInbox.receive(url)" in app
assert 'payload["externalProjectPath"]' in host
assert 'input.get("externalProjectPath")' in rust
assert 'menus::invoke(app, ctx, "file.open"' in rust
assert "external_project_open_ready()" in rust
assert "deliveredKeys" not in host, "Cancelled/failed files must be retryable"
assert "pendingExternalProjectPath: String?" not in host, "Handoffs must not overwrite each other"
assert "NSFileCoordinator" in host and "startAccessingSecurityScopedResource" in host
assert 'title: "Open Copy"' in host and 'title: "Cancel"' in host

if platform.system() == "Darwin":
    start = host.index("final class ExternalProjectInbox:")
    opening = host.index("{", start)
    depth = 1
    end = opening + 1
    while depth and end < len(host):
        depth += (host[end] == "{") - (host[end] == "}")
        end += 1
    assert depth == 0
    production_inbox = host[start:end]
    tests = r'''
let inbox = ExternalProjectInbox()
let first = URL(fileURLWithPath: "/tmp/Space 名.AEP")
let second = URL(fileURLWithPath: "/tmp/Native.unw")
var received: [URL] = []
inbox.receive(first)
inbox.receive(first)
inbox.receive(second)
assert(received.isEmpty)
inbox.register({ received.append($0) })
assert(received == [first, second], "Cold-launch queue is FIFO and deduplicates pending URLs")
inbox.receive(first)
assert(received == [first, second, first], "User can retry a cancelled file or reopen it")
inbox.unregister()
inbox.receive(second)
assert(received.count == 3)
inbox.register({ received.append($0) })
assert(received.count == 4 && received.last == second)
let overflow = ExternalProjectInbox()
for i in 0..<100 { overflow.receive(URL(fileURLWithPath: "/tmp/project-\(i).unw")) }
assert(overflow.pendingError != nil, "Overflow must not silently drop documents")
var kept: [URL] = []
var errors: [String] = []
overflow.register({ kept.append($0) }, onError: { errors.append($0) })
assert(!kept.isEmpty && kept.count < 100 && errors.count == 1)
assert(kept.first?.lastPathComponent == "project-0.unw")
print("ok: production Swift inbox cold/warm, FIFO, retry, reattach and visible overflow")
'''
    with tempfile.TemporaryDirectory(prefix="unwritten-handoff-") as tmp:
        script = Path(tmp) / "inbox.swift"
        script.write_text("import Foundation\nimport Combine\n" + production_inbox + "\n" + tests)
        subprocess.run(["swift", str(script)], check=True)
else:
    print("skip: production Swift inbox execution needs macOS/Combine")
print("ok: Files handoff contract from SwiftUI through worker to guarded Rust Open")
