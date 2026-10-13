import SwiftUI
import UniformTypeIdentifiers
import UIKit

private enum StudioPalette {
    static let background = Color(red: 0.055, green: 0.055, blue: 0.055)
    static let panel = Color(red: 0.085, green: 0.085, blue: 0.085)
    static let raised = Color(red: 0.12, green: 0.12, blue: 0.12)
    static let line = Color.white.opacity(0.10)
    static let secondary = Color.white.opacity(0.55)
    static let accent = Color(red: 0.91, green: 0.365, blue: 0.271)
}

struct WorkspaceView: View {
    @ObservedObject var model: StudioModel
    @State private var showOpenProject = false
    @State private var showImportMedia = false
    @State private var showSaveProject = false
    @State private var showDiscardPrompt = false
    @State private var showNewPrompt = false
    @State private var pendingProject: (Data, String)?

    var body: some View {
        GeometryReader { geometry in
            let wide = geometry.size.width >= 980 && geometry.size.height > 540
            VStack(spacing: 0) {
                topBar
                Rectangle().fill(StudioPalette.line).frame(height: 1)
                if !model.isReady {
                    loadingView
                } else if !model.hasProject {
                    welcomeView
                } else if wide {
                    landscapeWorkspace
                } else {
                    portraitWorkspace
                }
            }
            .background(StudioPalette.background.ignoresSafeArea())
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .fileImporter(isPresented: $showOpenProject,
                          allowedContentTypes: [.unwrittenStudioProject, .json, .xml],
                          allowsMultipleSelection: false,
                          onCompletion: handleProjectImport)
            .fileImporter(isPresented: $showImportMedia,
                          allowedContentTypes: [.image, .movie, .audio],
                          allowsMultipleSelection: false,
                          onCompletion: handleMediaImport)
            .fileExporter(isPresented: $showSaveProject,
                          document: model.projectDocumentData().map(ProjectDocument.init(data:)),
                          contentType: .unwrittenStudioProject,
                          defaultFilename: model.documentName,
                          onCompletion: handleSaveCompletion)
            .confirmationDialog("Discard current changes?", isPresented: $showDiscardPrompt,
                                titleVisibility: .visible) {
                Button("Discard and Open", role: .destructive) {
                    if let pendingProject {
                        model.openProject(data: pendingProject.0, displayName: pendingProject.1)
                    }
                    pendingProject = nil
                }
                Button("Cancel", role: .cancel) { pendingProject = nil }
            } message: {
                Text("The current project has unsaved edits. Save it before opening another project if you want to keep them.")
            }
            .confirmationDialog("Create a new project?", isPresented: $showNewPrompt,
                                titleVisibility: .visible) {
                Button("Discard and Create", role: .destructive) { model.createProject() }
                Button("Cancel", role: .cancel) { }
            } message: {
                Text("The current project has unsaved edits. Save it before starting over if you want to keep them.")
            }
            .alert("Unwritten Studio", isPresented: Binding(get: { model.notice != nil }, set: { if !$0 { model.dismissNotice() } })) {
                Button("OK", role: .cancel) { model.dismissNotice() }
            } message: {
                Text(model.notice ?? "")
            }
        }
        .background(StudioPalette.background)
    }

    private var topBar: some View {
        HStack(spacing: 14) {
            VStack(alignment: .leading, spacing: 1) {
                Text("UNWRITTEN")
                    .font(.system(size: 11, weight: .bold, design: .rounded))
                    .tracking(2.2)
                    .foregroundStyle(.white.opacity(0.62))
                Text(model.hasProject ? model.documentName : "STUDIO")
                    .font(.system(size: 18, weight: .semibold, design: .serif))
                    .lineLimit(1)
                    .foregroundStyle(.white)
            }
            Spacer(minLength: 4)
            if model.hasProject {
                if model.isImporting {
                    ProgressView().scaleEffect(0.7).tint(StudioPalette.accent)
                        .accessibilityLabel("Importing media")
                }
                HStack(spacing: 5) {
                    Circle().fill(model.isDirty ? StudioPalette.accent : Color.green.opacity(0.75)).frame(width: 6, height: 6)
                    Text(model.isDirty ? "EDITED" : "SAVED")
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .tracking(1.2)
                        .foregroundStyle(StudioPalette.secondary)
                }
            }
            Button { showOpenProject = true } label: {
                Image(systemName: "folder").frame(width: 38, height: 38)
            }
            .accessibilityLabel("Open project")
            .keyboardShortcut("o", modifiers: .command)
            .buttonStyle(ChromeButtonStyle())
            if model.hasProject {
                Button {
                    if model.isDirty { showNewPrompt = true }
                    else { model.createProject() }
                } label: {
                    Image(systemName: "doc.badge.plus").frame(width: 38, height: 38)
                }
                .accessibilityLabel("New project")
                .buttonStyle(ChromeButtonStyle())
                Button { showSaveProject = true } label: {
                    Image(systemName: "square.and.arrow.down").frame(width: 38, height: 38)
                }
                .accessibilityLabel("Save project")
                .keyboardShortcut("s", modifiers: .command)
                .buttonStyle(ChromeButtonStyle())
                Button { model.undo() } label: {
                    Image(systemName: "arrow.uturn.backward").frame(width: 38, height: 38)
                }
                .disabled(!(model.snapshot?.canUndo ?? false))
                .keyboardShortcut("z", modifiers: .command)
                .accessibilityLabel("Undo")
                .buttonStyle(ChromeButtonStyle())
                Button { model.redo() } label: {
                    Image(systemName: "arrow.uturn.forward").frame(width: 38, height: 38)
                }
                .disabled(!(model.snapshot?.canRedo ?? false))
                .keyboardShortcut("z", modifiers: [.command, .shift])
                .accessibilityLabel("Redo")
                .buttonStyle(ChromeButtonStyle())
            }
        }
        .padding(.horizontal, 18)
        .frame(height: 60)
        .background(StudioPalette.background)
    }

    private var loadingView: some View {
        VStack(spacing: 12) {
            ProgressView().tint(StudioPalette.accent)
            Text("Preparing local creative engine")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(StudioPalette.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var welcomeView: some View {
        VStack(alignment: .leading, spacing: 22) {
            Spacer()
            Text("Make room\nfor the idea.")
                .font(.system(size: 48, weight: .regular, design: .serif))
                .tracking(-1.5)
                .lineSpacing(-4)
                .foregroundStyle(.white)
            Text("A native iPad workspace for motion, image and composition.")
                .font(.system(size: 15))
                .foregroundStyle(StudioPalette.secondary)
                .frame(maxWidth: 390, alignment: .leading)
            HStack(spacing: 12) {
                Button { model.createProject() } label: {
                    Label("New Project", systemImage: "plus")
                        .font(.system(size: 14, weight: .semibold))
                        .padding(.horizontal, 18).padding(.vertical, 13)
                }
                .buttonStyle(AccentButtonStyle())
                Button { showOpenProject = true } label: {
                    Label("Open Project", systemImage: "folder")
                        .font(.system(size: 14, weight: .medium))
                        .padding(.horizontal, 18).padding(.vertical, 13)
                }
                .buttonStyle(ChromeButtonStyle())
            }
            Spacer()
            HStack {
                Text("LOCAL PROCESSING · NO REMOTE RENDER")
                Spacer()
                Text("iPadOS · RUST ENGINE")
            }
            .font(.system(size: 9, weight: .medium, design: .monospaced))
            .tracking(1)
            .foregroundStyle(.white.opacity(0.32))
        }
        .padding(.horizontal, 32)
        .padding(.vertical, 28)
        .frame(maxWidth: 720, maxHeight: .infinity, alignment: .leading)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var landscapeWorkspace: some View {
        HStack(spacing: 0) {
            projectBrowser
                .frame(width: 222)
            Rectangle().fill(StudioPalette.line).frame(width: 1)
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    toolRail
                    previewPanel
                    Rectangle().fill(StudioPalette.line).frame(width: 1)
                    inspectorPanel.frame(width: 258)
                }
                Rectangle().fill(StudioPalette.line).frame(height: 1)
                timelinePanel.frame(height: 224)
            }
        }
    }

    private var portraitWorkspace: some View {
        VStack(spacing: 0) {
            compositionStrip
                .frame(height: 54)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            previewPanel
                .frame(maxHeight: .infinity)
                .frame(minHeight: 210)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            inspectorPanel
                .frame(height: 105)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            timelinePanel
                .frame(height: max(205, min(260, UIScreen.main.bounds.height * 0.30)))
        }
    }

    private var projectBrowser: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionTitle("PROJECT", trailing: "\(model.snapshot?.items.count ?? 0)")
                .padding(.horizontal, 14).padding(.top, 16).padding(.bottom, 10)
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(model.snapshot?.items ?? []) { item in
                        Button {
                            if item.kind == "Composition" { model.openComposition(item.id) }
                        } label: {
                            HStack(spacing: 9) {
                                Image(systemName: item.kind == "Composition" ? "rectangle.on.rectangle" : item.kind == "Folder" ? "folder" : "square.fill")
                                    .font(.system(size: 13))
                                    .foregroundStyle(item.kind == "Composition" ? StudioPalette.accent : .white.opacity(0.55))
                                    .frame(width: 19)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(item.name).font(.system(size: 12, weight: .medium)).lineLimit(1)
                                    Text(item.kind.uppercased()).font(.system(size: 8, weight: .medium, design: .monospaced)).tracking(0.8).foregroundStyle(StudioPalette.secondary)
                                }
                                Spacer(minLength: 0)
                                if let layers = item.layerCount {
                                    Text("\(layers)").font(.system(size: 9, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
                                }
                            }
                            .padding(.horizontal, 9).padding(.vertical, 9)
                            .background(item.id == model.activeComposition?.id ? StudioPalette.raised : .clear)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .disabled(item.kind != "Composition")
                    }
                }
                .padding(.horizontal, 8)
            }
            Spacer(minLength: 8)
            mediaImportButton
                .padding(12)
        }
        .background(StudioPalette.panel)
    }

    private var compositionStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach((model.snapshot?.items ?? []).filter { $0.kind == "Composition" }) { item in
                    Button { model.openComposition(item.id) } label: {
                        HStack(spacing: 7) {
                            Rectangle().fill(item.id == model.activeComposition?.id ? StudioPalette.accent : .clear).frame(width: 2, height: 14)
                            Text(item.name).font(.system(size: 11, weight: .medium))
                            if let width = item.width, let height = item.height {
                                Text("\(width)×\(height)").font(.system(size: 8, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
                            }
                        }
                        .padding(.horizontal, 9).padding(.vertical, 7)
                        .background(StudioPalette.panel)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 14)
        }
        .background(StudioPalette.panel)
    }

    private var toolRail: some View {
        VStack(spacing: 8) {
            toolButton("cursorarrow", title: "Select", active: true) {}
            Rectangle().fill(StudioPalette.line).frame(height: 1).padding(.vertical, 5)
            toolButton("square", title: "Add shape") { model.addShape() }
            toolButton("textformat", title: "Add text") { model.addText() }
            toolButton("plus.rectangle.on.folder", title: "Import media") { showImportMedia = true }
            Spacer()
            Text("TOOLS").font(.system(size: 8, weight: .medium, design: .monospaced)).tracking(1).foregroundStyle(.white.opacity(0.3)).rotationEffect(.degrees(-90)).frame(height: 48)
        }
        .padding(.horizontal, 8).padding(.vertical, 14)
        .frame(width: 54)
        .background(StudioPalette.panel)
    }

    private var previewPanel: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(model.activeComposition?.name ?? "Composition")
                        .font(.system(size: 11, weight: .semibold))
                    Text(model.activeComposition.map { "\($0.width) × \($0.height)  ·  \(Int($0.frameRate.rounded())) fps" } ?? "NO ACTIVE COMPOSITION")
                        .font(.system(size: 8, weight: .medium, design: .monospaced))
                        .tracking(0.6)
                        .foregroundStyle(StudioPalette.secondary)
                }
                Spacer()
                if model.isRendering { ProgressView().scaleEffect(0.65).tint(StudioPalette.accent) }
                Text(model.snapshot?.renderer ?? "Local renderer")
                    .font(.system(size: 8, weight: .medium, design: .monospaced))
                    .foregroundStyle(StudioPalette.secondary)
                    .lineLimit(1)
            }
            .padding(.horizontal, 13).frame(height: 40)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            GeometryReader { geometry in
                ZStack {
                    Canvas { context, size in
                        let cell: CGFloat = 18
                        for row in 0..<Int(size.height / cell) + 1 {
                            for column in 0..<Int(size.width / cell) + 1 where (row + column).isMultiple(of: 2) {
                                let rect = CGRect(x: CGFloat(column) * cell, y: CGFloat(row) * cell, width: cell, height: cell)
                                context.fill(Path(rect), with: .color(Color.white.opacity(0.035)))
                            }
                        }
                    }
                    if let preview = model.preview {
                        Image(uiImage: preview)
                            .resizable()
                            .aspectRatio(contentMode: .fit)
                            .padding(18)
                            .frame(width: geometry.size.width, height: geometry.size.height)
                            .accessibilityLabel("Locally rendered composition preview")
                    } else {
                        VStack(spacing: 7) {
                            Image(systemName: "viewfinder").font(.system(size: 24, weight: .ultraLight))
                            Text(model.activeComposition == nil ? "OPEN A COMPOSITION" : "ADD A LAYER TO BEGIN")
                                .font(.system(size: 8, weight: .medium, design: .monospaced)).tracking(1.2)
                        }
                        .foregroundStyle(StudioPalette.secondary)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            HStack(spacing: 10) {
                Button { model.seek(to: 0, render: true) } label: { Image(systemName: "backward.end.fill") }
                    .accessibilityLabel("Go to start")
                Slider(value: Binding(get: { model.playheadSeconds }, set: { model.seek(to: $0) }),
                       in: 0...(model.activeComposition?.durationSeconds ?? 1),
                       onEditingChanged: { editing in if !editing { model.refreshPreview() } })
                    .tint(StudioPalette.accent)
                Text(timecode(model.playheadSeconds))
                    .font(.system(size: 9, weight: .medium, design: .monospaced))
                    .foregroundStyle(.white.opacity(0.72))
                    .frame(width: 66, alignment: .trailing)
            }
            .padding(.horizontal, 12).frame(height: 40)
        }
        .background(StudioPalette.background)
    }

    private var inspectorPanel: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionTitle("INSPECTOR", trailing: "LAYER")
                .padding(.horizontal, 14).padding(.vertical, 12)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            let selected = selectedLayer
            if let selected {
                HStack(spacing: 9) {
                    Rectangle().fill(StudioPalette.accent).frame(width: 2, height: 34)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(selected.name).font(.system(size: 13, weight: .semibold)).lineLimit(1)
                        Text(selected.kind.uppercased()).font(.system(size: 8, weight: .medium, design: .monospaced)).tracking(1).foregroundStyle(StudioPalette.secondary)
                    }
                    Spacer()
                    Text(String(format: "%02d", selected.stackIndex))
                        .font(.system(size: 11, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
                }
                .padding(.horizontal, 14).padding(.vertical, 12)
                keyValue("IN", value: timecode(selected.inSeconds))
                keyValue("OUT", value: timecode(selected.outSeconds))
                keyValue("DURATION", value: String(format: "%.2f s", selected.durationSeconds))
            } else {
                Text("Select a layer in the timeline to inspect it.")
                    .font(.system(size: 11)).foregroundStyle(StudioPalette.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                    .padding(14)
            }
            Spacer(minLength: 0)
        }
        .background(StudioPalette.panel)
    }

    private var timelinePanel: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                sectionTitle("TIMELINE", trailing: "\(model.activeComposition?.layers.count ?? 0) LAYERS")
                Spacer()
                Button { showImportMedia = true } label: { Image(systemName: "square.and.arrow.down.on.square").font(.system(size: 12)).foregroundStyle(.white.opacity(0.66)) }
                    .accessibilityLabel("Import media")
                Button { model.addSolid() } label: { Image(systemName: "plus").font(.system(size: 12, weight: .semibold)).foregroundStyle(StudioPalette.accent) }
                    .accessibilityLabel("Add solid layer")
            }
            .padding(.horizontal, 14).frame(height: 39)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            HStack(spacing: 0) {
                Text("LAYER").frame(width: 150, alignment: .leading)
                HStack {
                    Text("00:00"); Spacer(); Text(timecode((model.activeComposition?.durationSeconds ?? 0) / 2)); Spacer(); Text(timecode(model.activeComposition?.durationSeconds ?? 0))
                }
                .padding(.horizontal, 9)
            }
            .font(.system(size: 8, weight: .medium, design: .monospaced)).tracking(0.5).foregroundStyle(StudioPalette.secondary)
            .frame(height: 27).padding(.horizontal, 12)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            ScrollView(.vertical, showsIndicators: false) {
                VStack(spacing: 0) {
                    ForEach(model.activeComposition?.layers ?? []) { layer in
                        timelineRow(layer)
                        Rectangle().fill(StudioPalette.line.opacity(0.7)).frame(height: 1)
                    }
                    if model.activeComposition?.layers.isEmpty ?? true {
                        Text("No layers yet · use + to add a solid")
                            .font(.system(size: 10)).foregroundStyle(StudioPalette.secondary)
                            .frame(maxWidth: .infinity, alignment: .leading).padding(14)
                    }
                }
            }
        }
        .background(StudioPalette.panel)
    }

    private func timelineRow(_ layer: StudioLayer) -> some View {
        let isSelected = model.snapshot?.selectedLayers.contains(layer.id) ?? false
        let duration = max(model.activeComposition?.durationSeconds ?? 1, 0.001)
        return Button { model.selectLayer(layer.id) } label: {
            HStack(spacing: 0) {
                HStack(spacing: 8) {
                    Text(String(format: "%02d", layer.stackIndex))
                        .font(.system(size: 8, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
                        .frame(width: 19, alignment: .leading)
                    Image(systemName: iconFor(layer.kind)).font(.system(size: 9)).foregroundStyle(isSelected ? StudioPalette.accent : .white.opacity(0.45))
                    Text(layer.name).font(.system(size: 10, weight: isSelected ? .semibold : .regular)).lineLimit(1)
                    Spacer(minLength: 0)
                }
                .frame(width: 150, alignment: .leading)
                GeometryReader { geometry in
                    let width = geometry.size.width
                    let left = max(0, min(width, width * layer.inSeconds / duration))
                    let right = max(left + 12, min(width, width * layer.outSeconds / duration))
                    ZStack(alignment: .leading) {
                        Rectangle().fill(Color.white.opacity(0.035))
                        RoundedRectangle(cornerRadius: 2)
                            .fill(isSelected ? StudioPalette.accent : Color.white.opacity(0.22))
                            .frame(width: max(12, right - left), height: 17)
                            .offset(x: left)
                        Rectangle().fill(Color.white.opacity(0.35)).frame(width: 1, height: 27)
                            .offset(x: max(0, min(width - 1, width * model.playheadSeconds / duration)))
                    }
                }
                .frame(height: 30)
                .padding(.horizontal, 8)
            }
            .padding(.horizontal, 12).frame(height: 36)
            .background(isSelected ? Color.white.opacity(0.045) : .clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var selectedLayer: StudioLayer? {
        guard let id = model.snapshot?.selectedLayers.first else { return nil }
        return model.activeComposition?.layers.first(where: { $0.id == id })
    }

    private var mediaImportButton: some View {
        Button { showImportMedia = true } label: {
            HStack(spacing: 8) {
                Image(systemName: "plus.rectangle.on.folder")
                Text("IMPORT MEDIA").font(.system(size: 8, weight: .semibold, design: .monospaced)).tracking(1)
                Spacer()
            }
            .foregroundStyle(.white.opacity(0.72))
            .padding(.horizontal, 10).padding(.vertical, 11)
            .background(StudioPalette.raised)
        }
        .buttonStyle(.plain)
    }

    private func sectionTitle(_ title: String, trailing: String) -> some View {
        HStack {
            Text(title).foregroundStyle(.white.opacity(0.70))
            Spacer()
            Text(trailing).foregroundStyle(StudioPalette.secondary)
        }
        .font(.system(size: 8, weight: .semibold, design: .monospaced))
        .tracking(1.1)
    }

    private func keyValue(_ key: String, value: String) -> some View {
        HStack {
            Text(key).font(.system(size: 8, weight: .medium, design: .monospaced)).tracking(0.8).foregroundStyle(StudioPalette.secondary)
            Spacer()
            Text(value).font(.system(size: 9, design: .monospaced)).foregroundStyle(.white.opacity(0.78))
        }
        .padding(.horizontal, 14).padding(.vertical, 5)
    }

    private func toolButton(_ symbol: String, title: String, active: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 15, weight: .regular))
                .foregroundStyle(active ? StudioPalette.accent : .white.opacity(0.65))
                .frame(width: 36, height: 36)
                .background(active ? StudioPalette.accent.opacity(0.13) : .clear)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
    }

    private func iconFor(_ kind: String) -> String {
        switch kind {
        case "Text": return "textformat"
        case "Shape": return "scribble.variable"
        case "Solid": return "square.fill"
        case "Footage": return "film"
        case "3D Model", "3D Primitive": return "cube"
        default: return "square.on.square"
        }
    }

    private func timecode(_ seconds: Double) -> String {
        let safe = max(0, seconds.isFinite ? seconds : 0)
        let whole = Int(safe)
        let frames = Int((safe - Double(whole)) * 30)
        return String(format: "%02d:%02d:%02d:%02d", whole / 3600, (whole / 60) % 60, whole % 60, frames)
    }

    private func handleProjectImport(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            guard let url = urls.first else { return }
            do {
                let access = url.startAccessingSecurityScopedResource()
                defer { if access { url.stopAccessingSecurityScopedResource() } }
                let data = try Data(contentsOf: url)
                let name = url.deletingPathExtension().lastPathComponent
                if model.isDirty {
                    pendingProject = (data, name)
                    showDiscardPrompt = true
                } else {
                    model.openProject(data: data, displayName: name)
                }
            } catch {
                model.presentNotice("Could not open the selected project: \(error.localizedDescription)")
            }
        case .failure(let error):
            if (error as NSError).code != NSUserCancelledError {
                model.presentNotice("Could not open the selected project: \(error.localizedDescription)")
            }
        }
    }

    private func handleMediaImport(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls): if let url = urls.first { model.importMedia(from: url) }
        case .failure: break
        }
    }

    private func handleSaveCompletion(_ result: Result<URL, Error>) {
        switch result {
        case .success(let url): model.savedAs(url.lastPathComponent)
        case .failure(let error):
            if (error as NSError).code != NSUserCancelledError {
                model.presentNotice("Could not save the project: \(error.localizedDescription)")
            }
        }
    }
}

private struct ChromeButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(.white.opacity(configuration.isPressed ? 0.52 : 0.82))
            .background(configuration.isPressed ? StudioPalette.raised : .clear)
            .overlay(RoundedRectangle(cornerRadius: 3).stroke(StudioPalette.line, lineWidth: 1))
            .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

private struct AccentButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(.black.opacity(0.92))
            .background(configuration.isPressed ? StudioPalette.accent.opacity(0.76) : StudioPalette.accent)
            .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}
