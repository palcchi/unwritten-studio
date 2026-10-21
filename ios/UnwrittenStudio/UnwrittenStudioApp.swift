import SwiftUI
import UIKit

final class UnwrittenStudioAppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, supportedInterfaceOrientationsFor window: UIWindow?) -> UIInterfaceOrientationMask {
        // Apply the smoke orientation at the window root before presentation.
        // A child representable's geometry request alone can leave iPad in its
        // previous orientation. Normal launches retain every declared orientation.
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("--ui-smoke") {
            if arguments.contains("--smoke-portrait") { return .portrait }
            if arguments.contains("--smoke-landscape") { return .landscapeRight }
        }
        return UIDevice.current.userInterfaceIdiom == .pad ? .all : .allButUpsideDown
    }
}

@main
struct UnwrittenStudioApp: App {
    @UIApplicationDelegateAdaptor(UnwrittenStudioAppDelegate.self) private var appDelegate
    @StateObject private var externalProjectInbox = ExternalProjectInbox()

    var body: some Scene {
        WindowGroup {

            // The original Rust frontend owns every editor panel, menu and command.
            // UIKit only hosts its frames, input, keyboard and native file dialogs.
            OriginalWorkspaceView(externalProjectInbox: externalProjectInbox)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                // UIKit applies the notch/home-indicator safe area exactly once.
                // Do not first shrink the representable to SwiftUI's safe bounds.
                .ignoresSafeArea(.container)
                .preferredColorScheme(.dark)
                .onOpenURL { url in
                    // SwiftUI may deliver this before the representable/controller exists.
                    // The inbox retains it until the worker and its presentation surface are ready.
                    externalProjectInbox.receive(url)
                }
        }
    }
}
