import XCTest

/// Simulator acceptance only: XCUIDevice changes simulated hardware orientation,
/// unlike an app geometry request (which iPad windowing modes may reject).
final class WorkspaceOrientationTests: XCTestCase {
    func testPortrait() throws { try verifyWorkspace(portrait: true) }
    func testLandscape() throws { try verifyWorkspace(portrait: false) }

    private func verifyWorkspace(portrait: Bool) throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "works.valliant.unwrittenstudio")
        let orientation = portrait ? "portrait" : "landscape"
        app.launchArguments = ["--ui-smoke", "--smoke-\(orientation)"]
        // Launch in the opposite orientation, then rotate the running editor.
        // This exercises UIKit's transition/safe-area path, not just cold layout.
        XCUIDevice.shared.orientation = portrait ? .landscapeRight : .portrait
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30))
        XCUIDevice.shared.orientation = portrait ? .portrait : .landscapeRight
        let status = app.descendants(matching: .any).matching(identifier: "workspace-smoke-status").firstMatch
        let ready = NSPredicate { _, _ in
            guard status.exists, let value = status.value as? String else { return false }
            return value == "ready" || value.hasPrefix("error:")
        }
        let result = XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: ready, object: nil)], timeout: 120)
        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = "\(orientation)-workspace"
        attachment.lifetime = .keepAlways
        add(attachment)
        XCTAssertEqual(result, .completed, "Editor did not report ten rendered frames")
        XCTAssertEqual(status.value as? String, "ready", "Native editor reported a failure")
        XCTAssertEqual(app.state, .runningForeground, "Editor exited before capture")
        XCTAssertEqual(screenshot.image.size.height > screenshot.image.size.width, portrait,
                       "The actual simulator screen must match the requested orientation")
        XCTAssertEqual(app.frame.height > app.frame.width, portrait,
                       "The app window must match the requested orientation")
        app.terminate()
    }
}
