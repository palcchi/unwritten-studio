import SwiftUI

@main
struct UnwrittenStudioApp: App {
    var body: some Scene {
        WindowGroup {
            // The original Rust frontend owns every editor panel, menu and command.
            // UIKit only hosts its frames, input, keyboard and native file dialogs.
            OriginalWorkspaceView()
                .preferredColorScheme(.dark)
        }
    }
}
