import SwiftUI

@main
struct UnwrittenStudioApp: App {
    @StateObject private var model = StudioModel()

    var body: some Scene {
        WindowGroup {
            WorkspaceView(model: model)
                .preferredColorScheme(.dark)
                .tint(Color(red: 0.91, green: 0.365, blue: 0.271))
        }
    }
}
