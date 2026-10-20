import SwiftUI

@main
struct UnwrittenStudioApp: App {
    @StateObject private var model = StudioModel()

    var body: some Scene {
        WindowGroup {
            WorkspaceView(model: model)
                .preferredColorScheme(.dark)
                .tint(Color(red: 45 / 255, green: 140 / 255, blue: 235 / 255))
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
