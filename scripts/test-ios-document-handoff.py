#!/usr/bin/env python3
"""Check Files handoff wiring; on macOS execute the production Swift inbox.

These tests do not replace physical iPad Files/iCloud acceptance testing.
"""
from pathlib import Path
import argparse
import platform
import plistlib
import subprocess
import tempfile


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "--require-swift-runtime", action="store_true",
    help="Fail rather than report a source-only pass when macOS Swift/Combine is unavailable.",
)
args = parser.parse_args()


def extract_balanced_block(source: str, marker: str) -> str:
    """Extract the declaration beginning at *marker*, ignoring braces in literals/comments."""
    start = source.index(marker)
    opening = source.index("{", start)
    depth = 0
    index = opening
    state = "code"
    escaped = False
    while index < len(source):
        char = source[index]
        next_char = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if char == "/" and next_char == "/":
                state = "line-comment"
                index += 2
                continue
            if char == "/" and next_char == "*":
                state = "block-comment"
                index += 2
                continue
            if char == '"':
                state = "string"
                escaped = False
            elif char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    return source[start : index + 1]
        elif state == "string":
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                state = "code"
        elif state == "line-comment":
            if char == "\n":
                state = "code"
        elif state == "block-comment" and char == "*" and next_char == "/":
            state = "code"
            index += 2
            continue
        index += 1
    raise ValueError(f"unbalanced Swift block after {marker!r}")


# Regression coverage for the old two-character step: braces inside a string or
# comment must not affect the declaration boundary, and adjacent braces must be
# visited rather than skipped.
_extraction_fixture = 'prefix final class Example { let text = "}" /* { */ {} } suffix'
assert extract_balanced_block(_extraction_fixture, "final class Example") == (
    'final class Example { let text = "}" /* { */ {} }'
)


ROOT = Path(__file__).resolve().parents[1]
host = (ROOT / "ios/UnwrittenStudio/OriginalWorkspaceView.swift").read_text(encoding="utf-8")
app = (ROOT / "ios/UnwrittenStudio/UnwrittenStudioApp.swift").read_text(encoding="utf-8")
rust = (ROOT / "apps/unwritten-studio-core/src/native_ui.rs").read_text(encoding="utf-8")
with (ROOT / "ios/UnwrittenStudio/Info.plist").open("rb") as plist_file:
    info = plistlib.load(plist_file)
aep_type = "works.valliant.unwrittenstudio.aep"
assert any(
    declaration.get("UTTypeIdentifier") == aep_type
    and "aep" in declaration.get("UTTypeTagSpecification", {}).get("public.filename-extension", [])
    for declaration in info.get("UTImportedTypeDeclarations", [])
), "Files must recognize the AEP filename extension"
assert any(
    aep_type in declaration.get("LSItemContentTypes", [])
    and declaration.get("CFBundleTypeRole") == "Viewer"
    for declaration in info.get("CFBundleDocumentTypes", [])
), "The app must register as a non-editing AEP handler for Files"
assert "@StateObject private var externalProjectInbox" in app
assert ".onOpenURL" in app and "externalProjectInbox.receive(url)" in app
assert 'payload["externalProjectPath"]' in host
assert 'input.get("externalProjectPath")' in rust
assert 'menus::invoke(app, ctx, "file.open"' in rust
assert "external_project_open_ready()" in rust
# A terminal failure or Cancel finishes that attempt; never automatically reopen
# its alert every frame. A fresh user delivery of the same URL must be accepted.
assert "deliveredKeys" not in host, "A fresh Files request must be able to retry a cancelled/failed file"
assert "pendingExternalProjectPath: String?" not in host, "Handoffs must not overwrite each other"
assert "NSFileCoordinator" in host and "startAccessingSecurityScopedResource" in host
assert "coordinate(readingItemAt: fileURL" in host
assert "copyItem(at: coordinatedURL, to: destination)" in host
assert 'appendingPathComponent("External Projects", isDirectory: true)' in host
assert "stagedExternalProjectURLs" in host
assert 'title: "Open Copy"' in host and 'title: "Cancel"' in host
assert 'if extensionName == "unw"' in host, "Native .unw must retain its real Save path, never a disposable staged path"
assert "controller.dismiss(animated: true)" in host and "self?.confirmAEPImport(projectURL, id: id)" in host
assert "self.stageExternalAEP(url)" in host, "The in-app Open picker must use the same coordinated AEP read as Files"
assert "private static let maximumPendingURLs = 64" in host
assert "private var pendingAEPStaging = 0" in host
assert "stagedExternalProjectURLs.count + pendingAEPStaging < Self.maximumPendingExternalProjects" in host
assert "private var receiver: ((URL) -> Bool)?" in host
assert "private var flushing = false" in host
assert "func retryDelivery() { flush() }" in host
assert "func unregister() { receiver = nil; errorReceiver = nil }" in host
assert "guard receiver(url) else { return }" in host
assert "private final class ExternalProjectBuffer" in host
assert "private let externalProjectPaths = ExternalProjectBuffer()" in host
assert "func enqueueExternalProject(path: String) -> Bool" in host
assert "var canQueueExternalProject: Bool { externalProjectPaths.hasCapacity }" in host
assert "externalProjectPaths.popFirst()" in host
assert "max(0, min(8, result.output?.externalProjectCapacity ?? 0))" in host
assert "discardStagedAEP" in host and "self?.discardStagedAEP(url)" in host
assert "sessionDirectory(in root: URL)" in host and "where child.lastPathComponent != sessionName" in host
assert "defer { if access { fileURL.stopAccessingSecurityScopedResource() } }" in host
assert "stagedExternalProjectURLs.contains(fileURL)" in host
assert "stagedExternalProjectURLs.contains(url) || retainProjectURL(url)" in host

if platform.system() == "Darwin":
    production_inbox = extract_balanced_block(host, "final class ExternalProjectInbox:")
    production_buffer = extract_balanced_block(host, "private final class ExternalProjectBuffer")
    staging_start = host.index("private enum ExternalAEPStaging")
    staging_end = host.index("/// The iOS host", staging_start)
    production_staging = host[staging_start:staging_end]
    tests = r'''
let inbox = ExternalProjectInbox()
let first = URL(fileURLWithPath: "/tmp/Space 名.AEP")
let second = URL(fileURLWithPath: "/tmp/Native.unw")
var received: [URL] = []
inbox.receive(first)
inbox.receive(first)
inbox.receive(second)
assert(received.isEmpty)
inbox.register({ received.append($0); return true })
assert(received == [first, second], "Cold-launch queue is FIFO and deduplicates pending URLs")
inbox.receive(first)
assert(received == [first, second, first], "User can retry a cancelled file or reopen it")
inbox.unregister()
inbox.receive(second)
assert(received.count == 3)
inbox.register({ received.append($0); return true })
assert(received.count == 4 && received.last == second)
let overflow = ExternalProjectInbox()
for i in 0..<100 { overflow.receive(URL(fileURLWithPath: "/tmp/project-\(i).unw")) }
assert(overflow.pendingError != nil, "Overflow must not silently drop documents")
var kept: [URL] = []
var errors: [String] = []
overflow.register({ kept.append($0); return true }, onError: { errors.append($0) })
assert(!kept.isEmpty && kept.count < 100 && errors.count == 1)
assert(kept.first?.lastPathComponent == "project-0.unw")
print("ok: production Swift inbox cold/warm, FIFO, retry, reattach and visible overflow")
let exact = ExternalProjectInbox()
for i in 0..<64 { exact.receive(URL(fileURLWithPath: "/tmp/exact-\(i).aep")) }
assert(exact.pendingError == nil, "Exactly 64 queued URLs must be accepted")
var exactReceived: [URL] = []
exact.register({ exactReceived.append($0); return true })
assert(exactReceived.count == 64 && exactReceived.first?.lastPathComponent == "exact-0.aep")
let blocked = ExternalProjectInbox()
var blockedCalls = 0
var allowBlocked = false
var blockedReceived: [URL] = []
blocked.receive(first)
blocked.receive(second)
blocked.register({ url in
    blockedCalls += 1
    guard allowBlocked else { return false }
    blockedReceived.append(url)
    return true
})
assert(blockedCalls == 1 && blockedReceived.isEmpty, "A blocked receiver must keep the FIFO head")
allowBlocked = true
blocked.retryDelivery()
assert(blockedReceived == [first, second], "Retry must drain the retained FIFO in order")
let reentrant = ExternalProjectInbox()
let reentrantFirst = URL(fileURLWithPath: "/tmp/reentrant-first.aep")
let reentrantSecond = URL(fileURLWithPath: "/tmp/reentrant-second.aep")
var reentrantReceived: [URL] = []
var didReenter = false
reentrant.register({ url in
    reentrantReceived.append(url)
    if !didReenter {
        didReenter = true
        reentrant.receive(reentrantSecond)
        reentrant.unregister()
    }
    return true
})
reentrant.receive(reentrantFirst)
assert(reentrantReceived == [reentrantFirst], "Reentrant delivery must not recurse")
reentrant.register({ reentrantReceived.append($0); return true })
assert(reentrantReceived == [reentrantFirst, reentrantSecond], "Unregister must interrupt flushing and retry must resume FIFO")
print("ok: production Swift inbox cold/warm, FIFO, retry, reattach, exact capacity, backpressure and reentrancy")
private let buffer = ExternalProjectBuffer(capacity: 64)
for i in 0..<64 { assert(buffer.append("/tmp/buffer-\(i).aep")) }
assert(!buffer.hasCapacity && !buffer.append("/tmp/overflow.aep"), "External project buffer must reject its 65th item")
for i in 0..<64 { assert(buffer.popFirst() == "/tmp/buffer-\(i).aep", "External project buffer must remain FIFO") }
assert(buffer.popFirst() == nil)
private let rapid = ExternalProjectBuffer(capacity: 8)
for i in 0..<8 { assert(rapid.append("/tmp/rapid-\(i).aep")) }
assert(!rapid.append("/tmp/rapid-8.aep"), "The ninth path must remain with the caller until capacity returns")
assert(rapid.popFirst() == "/tmp/rapid-0.aep")
assert(rapid.append("/tmp/rapid-8.aep"), "A rejected ninth path can retry after a slot is released")
assert((1...8).allSatisfy { rapid.popFirst() == "/tmp/rapid-\($0).aep" } && rapid.popFirst() == nil, "The retry must preserve all nine paths in FIFO order")
private let zero = ExternalProjectBuffer(capacity: 0)
assert(!zero.hasCapacity && !zero.append("/tmp/no-capacity.aep") && zero.popFirst() == nil)
print("ok: thread-safe external project buffer exact capacity, FIFO, rapid-send retention and zero capacity")
let fm = FileManager.default
let root = fm.temporaryDirectory.appendingPathComponent("unwritten-aep-staging-\(UUID().uuidString)", isDirectory: true)
try fm.createDirectory(at: root, withIntermediateDirectories: true)
defer { try? fm.removeItem(at: root) }
let source = root.appendingPathComponent("Space 名.AEP")
let destination = root.appendingPathComponent("App Support/External Projects", isDirectory: true)
let payload = Data("RIFX-test-source".utf8)
try payload.write(to: source)
let staged = try ExternalAEPStaging.copy(source, into: destination)
assert(staged != source && staged.deletingLastPathComponent() == destination)
let stagedBytes = try Data(contentsOf: staged)
let originalBytes = try Data(contentsOf: source)
assert(stagedBytes == payload && originalBytes == payload, "Copy must not alter the source")
let another = try ExternalAEPStaging.copy(source, into: destination)
assert(staged != another, "Repeated opens need independent staged copies")
try fm.removeItem(at: source)
let durableBytes = try Data(contentsOf: staged)
assert(durableBytes == payload, "Rust must not depend on the source after coordinated copying")
let unw = root.appendingPathComponent("Native.unw")
try payload.write(to: unw)
do {
    _ = try ExternalAEPStaging.copy(unw, into: destination)
    assertionFailure("Staging native Save destinations is forbidden")
} catch {}
let large = root.appendingPathComponent("Large.aep")
try payload.write(to: large)
do {
    _ = try ExternalAEPStaging.copy(large, into: destination, maximumBytes: 4)
    assertionFailure("Oversized imports must be rejected before copying")
} catch {}
do {
    _ = try ExternalAEPStaging.copy(source, into: destination)
    assertionFailure("Missing provider documents must return an error")
} catch {}
let copies = try fm.contentsOfDirectory(at: destination, includingPropertiesForKeys: nil)
assert(copies.count == 2, "Failed operations must not leave partial staged files")
print("ok: production coordinated AEP staging, durable copy, Unicode, retry, size/missing errors and UNW safety")
let boundary = root.appendingPathComponent("Boundary.aep")
try payload.write(to: boundary)
let exactCopy = try ExternalAEPStaging.copy(boundary, into: destination, maximumBytes: payload.count)
let exactAttributes = try fm.attributesOfItem(atPath: exactCopy.path)
assert(exactAttributes[.size] as? Int == payload.count)
let folder = root.appendingPathComponent("Folder.aep", isDirectory: true)
try fm.createDirectory(at: folder, withIntermediateDirectories: true)
do {
    _ = try ExternalAEPStaging.copy(folder, into: destination)
    assertionFailure("An AEP-named directory is not an importable regular file")
} catch {}
do {
    _ = try ExternalAEPStaging.copy(URL(string: "https://example.invalid/Project.aep")!, into: destination)
    assertionFailure("Remote URLs must not bypass local provider coordination")
} catch {}
let blockedDestination = root.appendingPathComponent("Not a directory")
try payload.write(to: blockedDestination)
do {
    _ = try ExternalAEPStaging.copy(boundary, into: blockedDestination)
    assertionFailure("A failed staging destination must return an error")
} catch {}
let boundaryBytes = try Data(contentsOf: boundary)
assert(boundaryBytes == payload, "Even a failed staging attempt must leave the source unchanged")
let finalCopies = try fm.contentsOfDirectory(at: destination, includingPropertiesForKeys: nil)
assert(finalCopies.count == 3, "Rejected directories, remote URLs and destination errors must not add copies")
print("ok: exact size boundary, directory/remote rejection, destination error and unchanged source")
let sessionRoot = root.appendingPathComponent("Sessions", isDirectory: true)
let nativeOutsideRoot = root.deletingLastPathComponent().appendingPathComponent("Native-save-\(UUID().uuidString).unw")
try Data("native-save".utf8).write(to: nativeOutsideRoot)
let currentSession = try ExternalAEPStaging.sessionDirectory(in: sessionRoot)
let currentMarker = currentSession.appendingPathComponent("keep.aep")
try Data("current".utf8).write(to: currentMarker)
let oldSession = sessionRoot.appendingPathComponent(UUID().uuidString, isDirectory: true)
try fm.createDirectory(at: oldSession, withIntermediateDirectories: true)
try Data("old".utf8).write(to: oldSession.appendingPathComponent("old.aep"))
try Data("unwritten-aep-staging-v1\n2147483647\n".utf8).write(to: oldSession.appendingPathComponent(".unwritten-aep-staging-owner"))
let liveSession = sessionRoot.appendingPathComponent(UUID().uuidString, isDirectory: true)
try fm.createDirectory(at: liveSession, withIntermediateDirectories: true)
try Data("unwritten-aep-staging-v1\n\(ProcessInfo.processInfo.processIdentifier)\n".utf8).write(to: liveSession.appendingPathComponent(".unwritten-aep-staging-owner"))
let malformedSession = sessionRoot.appendingPathComponent(UUID().uuidString, isDirectory: true)
try fm.createDirectory(at: malformedSession, withIntermediateDirectories: true)
try Data("not-an-owner-marker\n".utf8).write(to: malformedSession.appendingPathComponent(".unwritten-aep-staging-owner"))
let unownedSession = sessionRoot.appendingPathComponent(UUID().uuidString, isDirectory: true)
try fm.createDirectory(at: unownedSession, withIntermediateDirectories: true)
let oldLegacy = sessionRoot.appendingPathComponent("\(UUID().uuidString)-legacy.AEP")
try Data("legacy".utf8).write(to: oldLegacy)
let unrelatedUnw = sessionRoot.appendingPathComponent("Native.unw")
try Data("native-in-root".utf8).write(to: unrelatedUnw)
let unknownFolder = sessionRoot.appendingPathComponent("do-not-delete", isDirectory: true)
try fm.createDirectory(at: unknownFolder, withIntermediateDirectories: true)
let symlink = sessionRoot.appendingPathComponent("old-link.aep")
try fm.createSymbolicLink(at: symlink, withDestinationURL: oldLegacy)
let sameSession = try ExternalAEPStaging.sessionDirectory(in: sessionRoot)
assert(sameSession == currentSession, "A process session directory must survive controller recreation")
assert(fm.fileExists(atPath: currentMarker.path), "Current-session staged files must survive session reuse")
assert(!fm.fileExists(atPath: oldSession.path), "Stale marked prior-session residue must be pruned")
assert(fm.fileExists(atPath: liveSession.path) && fm.fileExists(atPath: malformedSession.path) && fm.fileExists(atPath: unownedSession.path), "Live, malformed and unowned UUID sessions must survive cleanup")
assert(fm.fileExists(atPath: oldLegacy.path), "Legacy UUID*.aep files must survive cleanup")
assert(fm.fileExists(atPath: unrelatedUnw.path) && fm.fileExists(atPath: unknownFolder.path), "Unrelated native and unknown entries must survive cleanup")
assert(fm.fileExists(atPath: symlink.path), "Symlink residue must not be followed or deleted")
let nativeOutsideBytes = try Data(contentsOf: nativeOutsideRoot)
assert(nativeOutsideBytes == Data("native-save".utf8), "Native Save files outside the staging root must remain unchanged")
try fm.removeItem(at: nativeOutsideRoot)
print("ok: selective session residue cleanup preserves current session, unrelated entries, symlinks and native Save files")
'''
    with tempfile.TemporaryDirectory(prefix="unwritten-handoff-") as tmp:
        script = Path(tmp) / "inbox.swift"
        script.write_text("import Foundation\nimport Darwin\nimport Combine\n" + production_inbox + "\n" + production_buffer + "\n" + production_staging + "\n" + tests)
        subprocess.run(["swift", str(script)], check=True)
else:
    print("skip: production Swift inbox and coordinated-copy execution need macOS/Combine")
    if args.require_swift_runtime:
        parser.exit(2, "blocked: required Apple Swift runtime checks did not run; source checks are not acceptance evidence\n")
print("ok: Files handoff SOURCE contract from SwiftUI through worker to guarded Rust Open")
print("not covered: UIKit URL delivery, provider permissions, physical-iPad Files or touch acceptance")
