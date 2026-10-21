import SwiftUI

@main
struct UnwrittenStudioApp: App {
    var body: some Scene {
        WindowGroup {
            // The original Rust frontend owns every editor panel, menu and command.
            // UIKit only hosts its frames, input, keyboard and native file dialogs.
            OriginalWorkspaceView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                // UIKit applies the notch/home-indicator safe area exactly once.
                // Do not first shrink the representable to SwiftUI's safe bounds.
                .ignoresSafeArea(.container)
                .preferredColorScheme(.dark)
        }
    }
}
