import SwiftUI

@main
struct UnwrittenStudioApp: App {
    @StateObject private var model = StudioModel()

    var body: some Scene {
        WindowGroup {
            WorkspaceView(model: model)
                .preferredColorScheme(.dark)
                .tint(Color(red: 0.22, green: 0.52, blue: 0.96))
                .onChange(of: model.isReady) { _, ready in
                    // Explicit CI argument only; normal launches never create a document.
                    if ready && ProcessInfo.processInfo.arguments.contains("--ui-smoke") {
                        model.createProject(name: "Workspace Check")
                        model.addSolid()
                    }
                }
        }
    }
}
