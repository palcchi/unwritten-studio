import SwiftUI
import UniformTypeIdentifiers
import UIKit

private enum CompactPanel: String, CaseIterable {
    case canvas = "Canvas"
    case project = "Project"
    case effects = "Effects"
    case timeline = "Timeline"
}

private enum StudioPalette {
    static let background = Color(red: 0.055, green: 0.055, blue: 0.055)
    static let panel = Color(red: 0.085, green: 0.085, blue: 0.085)
    static let raised = Color(red: 0.14, green: 0.14, blue: 0.14)
    static let line = Color.white.opacity(0.10)
    static let secondary = Color.white.opacity(0.55)
    static let accent = Color(red: 0.22, green: 0.52, blue: 0.96)
}

struct WorkspaceView: View {
    @ObservedObject var model: StudioModel
    @ScaledMetric(relativeTo: .body) private var baseBodyPointSize: CGFloat = 16
    @ScaledMetric(relativeTo: .body) private var controlTargetSize: CGFloat = 44
    @State private var showOpenProject = false
    @State private var showImportMedia = false
    @State private var showSaveProject = false
    @State private var exportDocument: ProjectDocument?
    @State private var showDiscardPrompt = false
    @State private var showNewPrompt = false
    @State private var pendingProject: (Data, String)?
    @State private var isCanvasFocused = false
    @State private var compactPanel: CompactPanel = .canvas

    private var typeScale: CGFloat { baseBodyPointSize / 16 }

    var body: some View {
        GeometryReader { geometry in
            let wide = geometry.size.width >= 980
                && geometry.size.height > 540
                && geometry.size.width >= geometry.size.height * 1.15
            VStack(spacing: 0) {
                topBar
                Rectangle().fill(StudioPalette.line).frame(height: 1)
                if !model.isReady {
                    loadingView
                } else if !model.hasProject {
                    welcomeView
                } else if isCanvasFocused {
                    focusedWorkspace(size: geometry.size, wide: wide)
                } else if wide {
                    landscapeWorkspace(size: geometry.size)
                } else {
                    portraitWorkspace(size: geometry.size)
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
                          document: exportDocument,
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
        HStack(spacing: 8) {
            HStack(spacing: 8) {
                Text("UNWRITTEN STUDIO")
                    .font(.system(size: 12 * typeScale, weight: .bold))
                    .foregroundStyle(.white.opacity(0.82))
                    .lineLimit(1)
                if model.hasProject {
                    Rectangle().fill(StudioPalette.line).frame(width: 1, height: 18)
                    Text(model.documentName)
                        .font(.system(size: 13 * typeScale, weight: .medium))
                        .lineLimit(1)
                        .foregroundStyle(.white.opacity(0.72))
                }
            }
            .layoutPriority(1)
            Spacer(minLength: 4)
            if model.hasProject {
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: 2) {
                        fileMenu
                        editMenu
                        layerMenu
                        viewMenu
                    }
                    overflowMenu
                }
                if model.isImporting { ProgressView().scaleEffect(0.65).tint(StudioPalette.accent) }
                Circle().fill(model.isDirty ? StudioPalette.accent : Color.green.opacity(0.75)).frame(width: 6, height: 6)
                    .accessibilityLabel(model.isDirty ? "Unsaved changes" : "Saved")
            }
        }
        .padding(.horizontal, 12)
        .frame(height: max(52, controlTargetSize + 8))
        .background(StudioPalette.panel)
    }

    private var fileMenu: some View {
        Menu {
            Button("New Project", systemImage: "doc.badge.plus") { beginNewProject() }
                .keyboardShortcut("n", modifiers: .command)
            Button("Open Project…", systemImage: "folder") { showOpenProject = true }
                .keyboardShortcut("o", modifiers: .command)
            Button("Import Media…", systemImage: "plus.rectangle.on.folder") { showImportMedia = true }
            Divider()
            Button("Save Project", systemImage: "square.and.arrow.down") { beginSaveProject() }
                .keyboardShortcut("s", modifiers: .command)
        } label: {
            Label("File", systemImage: "doc")
                .padding(.horizontal, 8).frame(minHeight: controlTargetSize)
        }
        .foregroundStyle(.white.opacity(0.72))
    }

    private var editMenu: some View {
        Menu {
            Button("Undo", systemImage: "arrow.uturn.backward") { model.undo() }
                .disabled(!(model.snapshot?.canUndo ?? false)).keyboardShortcut("z", modifiers: .command)
            Button("Redo", systemImage: "arrow.uturn.forward") { model.redo() }
                .disabled(!(model.snapshot?.canRedo ?? false)).keyboardShortcut("z", modifiers: [.command, .shift])
        } label: {
            Label("Edit", systemImage: "pencil")
                .padding(.horizontal, 8).frame(minHeight: controlTargetSize)
        }
        .foregroundStyle(.white.opacity(0.72))
    }

    private var layerMenu: some View {
        Menu {
            Button("Solid", systemImage: "square.fill") { model.addSolid() }
            Button("Shape", systemImage: "scribble.variable") { model.addShape() }
            Button("Text", systemImage: "textformat") { model.addText() }
        } label: {
            Label("Layer", systemImage: "square.on.square")
                .padding(.horizontal, 8).frame(minHeight: controlTargetSize)
        }
        .foregroundStyle(.white.opacity(0.72))
    }

    private var viewMenu: some View {
        Menu {
            Button(isCanvasFocused ? "Show Workspace" : "Focus Composition", systemImage: isCanvasFocused ? "rectangle.on.rectangle" : "viewfinder") {
                withAnimation(.easeInOut(duration: 0.2)) { isCanvasFocused.toggle() }
            }
        } label: {
            Label("View", systemImage: isCanvasFocused ? "rectangle.on.rectangle" : "viewfinder")
                .padding(.horizontal, 8).frame(minHeight: controlTargetSize)
        }
        .foregroundStyle(.white.opacity(0.72))
    }

    private var overflowMenu: some View {
        Menu {
            fileMenu
            editMenu
            layerMenu
            viewMenu
        } label: {
            Image(systemName: "ellipsis.circle")
                .frame(width: controlTargetSize, height: controlTargetSize)
                .accessibilityLabel("Workspace menu")
        }
        .foregroundStyle(.white.opacity(0.82))
    }

    private func beginNewProject() {
        if model.isDirty { showNewPrompt = true } else { model.createProject() }
    }

    private var loadingView: some View {
        VStack(spacing: 12) {
            ProgressView().tint(StudioPalette.accent)
            Text("Preparing local creative engine")
                .font(.system(size: 14 * typeScale, design: .monospaced))
                .foregroundStyle(StudioPalette.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var welcomeView: some View {
        VStack(alignment: .leading, spacing: 22) {
            Spacer()
            Text("START A PROJECT")
                .font(.system(size: 28 * typeScale, weight: .semibold))
                .tracking(-1.5)
                .lineSpacing(-4)
                .foregroundStyle(.white)
            Text("A native workspace for motion, image and composition.")
                .font(.system(size: 17 * typeScale))
                .foregroundStyle(StudioPalette.secondary)
                .frame(maxWidth: 390, alignment: .leading)
            HStack(spacing: 12) {
                Button { model.createProject() } label: {
                    Label("New Project", systemImage: "plus")
                        .font(.system(size: 16 * typeScale, weight: .semibold))
                        .padding(.horizontal, 18).padding(.vertical, 13)
                }
                .buttonStyle(AccentButtonStyle())
                Button { showOpenProject = true } label: {
                    Label("Open Project", systemImage: "folder")
                        .font(.system(size: 16 * typeScale, weight: .medium))
                        .padding(.horizontal, 18).padding(.vertical, 13)
                }
                .buttonStyle(ChromeButtonStyle())
            }
            Spacer()
            HStack {
                Text("LOCAL PROCESSING · NO REMOTE RENDER")
                Spacer()
                Text("iOS · RUST ENGINE")
            }
            .font(.system(size: 11 * typeScale, weight: .medium, design: .monospaced))
            .tracking(1)
            .foregroundStyle(.white.opacity(0.32))
        }
        .padding(.horizontal, 32)
        .padding(.vertical, 28)
        .frame(maxWidth: 720, maxHeight: .infinity, alignment: .leading)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func landscapeWorkspace(size: CGSize) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                projectBrowser.frame(width: 248)
                Rectangle().fill(StudioPalette.line).frame(width: 1)
                toolRail
                previewPanel
                Rectangle().fill(StudioPalette.line).frame(width: 1)
                inspectorPanel.frame(width: max(284, controlTargetSize * 2 + 196))
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            timelinePanel.frame(height: min(280, max(220, size.height * 0.30)))
        }
    }

    private func portraitWorkspace(size: CGSize) -> some View {
        let veryCompact = size.height < 600
        let compact = size.height < 740
        let timelineHeight = veryCompact ? min(188, max(132, size.height * 0.30)) : min(compact ? 238 : 300, max(compact ? 190 : 220, size.height * 0.25))
        return VStack(spacing: 0) {
            if size.width < 600 {
                compactPanelSelector
                Rectangle().fill(StudioPalette.line).frame(height: 1)
                Group {
                    switch compactPanel {
                    case .canvas: previewPanel
                    case .project: projectBrowser
                    case .effects: inspectorPanel
                    case .timeline: timelinePanel
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                compositionStrip.frame(height: veryCompact ? 48 : 60)
                Rectangle().fill(StudioPalette.line).frame(height: 1)
                previewPanel.frame(maxHeight: .infinity).frame(minHeight: veryCompact ? 100 : (compact ? 160 : 220))
                Rectangle().fill(StudioPalette.line).frame(height: 1)
                inspectorPanel.frame(height: compact ? 184 : 214)
                Rectangle().fill(StudioPalette.line).frame(height: 1)
                timelinePanel.frame(height: timelineHeight)
            }
        }
    }

    private var compactPanelSelector: some View {
        HStack(spacing: 2) {
            ForEach(CompactPanel.allCases, id: \.self) { panel in
                Button { compactPanel = panel } label: {
                        Text(panel.rawValue)
                            .font(.system(size: 11 * typeScale, weight: .medium))
                        .foregroundStyle(compactPanel == panel ? .white : StudioPalette.secondary)
                        .frame(maxWidth: .infinity, minHeight: controlTargetSize)
                        .background(compactPanel == panel ? StudioPalette.accent.opacity(0.18) : .clear)
                        .overlay(alignment: .bottom) { Rectangle().fill(compactPanel == panel ? StudioPalette.accent : .clear).frame(height: 2) }
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Show \(panel.rawValue) panel")
            }
        }
        .padding(.horizontal, 6)
        .background(StudioPalette.panel)
    }

    private func focusedWorkspace(size: CGSize, wide: Bool) -> some View {
        VStack(spacing: 0) {
            if !wide {
                compositionStrip
                Rectangle().fill(StudioPalette.line).frame(height: 1)
            }
            HStack(spacing: 0) {
                if wide {
                    toolRail
                    Rectangle().fill(StudioPalette.line).frame(width: 1)
                }
                previewPanel
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            timelinePanel
                .frame(height: wide
                    ? min(280, max(220, size.height * 0.30))
                    : min(220, max(160, size.height * 0.22)))
        }
    }

    private var projectBrowser: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionTitle("Project", trailing: "\(model.snapshot?.items.count ?? 0)")
                .padding(.horizontal, 14).padding(.top, 16).padding(.bottom, 10)
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(model.snapshot?.items ?? []) { item in
                        Button {
                            if item.kind == "Composition" { model.openComposition(item.id) }
                        } label: {
                            HStack(spacing: 9) {
                                Image(systemName: item.kind == "Composition" ? "rectangle.on.rectangle" : item.kind == "Folder" ? "folder" : "square.fill")
                                    .font(.system(size: 15 * typeScale))
                                    .foregroundStyle(item.kind == "Composition" ? StudioPalette.accent : .white.opacity(0.55))
                                    .frame(width: 19)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(item.name).font(.system(size: 14 * typeScale, weight: .medium)).lineLimit(1)
                                    Text(item.kind).font(.system(size: 10 * typeScale, weight: .medium)).foregroundStyle(StudioPalette.secondary)
                                }
                                Spacer(minLength: 0)
                                if let layers = item.layerCount {
                                    Text("\(layers)").font(.system(size: 11 * typeScale, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
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
                            Text(item.name).font(.system(size: 13 * typeScale, weight: .medium))
                            if let width = item.width, let height = item.height {
                                Text("\(width)×\(height)").font(.system(size: 10 * typeScale, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
                            }
                        }
                        .padding(.horizontal, 12).padding(.vertical, 11)
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
                    Text("Tools").font(.system(size: 10 * typeScale, weight: .medium)).foregroundStyle(.white.opacity(0.3)).rotationEffect(.degrees(-90)).frame(height: 48)
        }
        .padding(.horizontal, 8).padding(.vertical, 14)
        .frame(width: max(64, controlTargetSize + 20))
        .background(StudioPalette.panel)
    }

    private var previewPanel: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(model.activeComposition?.name ?? "Composition")
                        .font(.system(size: 13 * typeScale, weight: .semibold))
                    Text(model.activeComposition.map { "\($0.width) × \($0.height)  ·  \(Int($0.frameRate.rounded())) fps" } ?? "NO ACTIVE COMPOSITION")
                        .font(.system(size: 10 * typeScale, weight: .medium, design: .monospaced))
                        .tracking(0.6)
                        .foregroundStyle(StudioPalette.secondary)
                }
                Spacer()
                if model.isRendering { ProgressView().scaleEffect(0.65).tint(StudioPalette.accent) }
                Text(model.snapshot?.renderer ?? "Local renderer")
                    .font(.system(size: 10 * typeScale, weight: .medium, design: .monospaced))
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
                            Image(systemName: "viewfinder").font(.system(size: 28 * typeScale, weight: .ultraLight))
                            Text(model.activeComposition == nil ? "OPEN A COMPOSITION" : "ADD A LAYER TO BEGIN")
                                .font(.system(size: 10 * typeScale, weight: .medium, design: .monospaced)).tracking(1.2)
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
                       onEditingChanged: { editing in if !editing { model.refreshSnapshot(render: true) } })
                    .tint(StudioPalette.accent)
                Text(timecode(model.playheadSeconds))
                    .font(.system(size: 11 * typeScale, weight: .medium, design: .monospaced))
                    .foregroundStyle(.white.opacity(0.72))
                    .frame(width: 66, alignment: .trailing)
            }
            .padding(.horizontal, 12).frame(height: 40)
        }
        .background(StudioPalette.background)
    }

    private var inspectorPanel: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 0) {
                sectionTitle("Effect Controls", trailing: "Layer")
                    .padding(.horizontal, 14).padding(.vertical, 12)
                Rectangle().fill(StudioPalette.line).frame(height: 1)
                let selected = selectedLayer
                if let selected {
                    HStack(spacing: 9) {
                        Rectangle().fill(StudioPalette.accent).frame(width: 2, height: 34)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(selected.name).font(.system(size: 15 * typeScale, weight: .semibold)).lineLimit(1)
                            Text(selected.kind).font(.system(size: 10 * typeScale, weight: .medium)).foregroundStyle(StudioPalette.secondary)
                        }
                        Spacer()
                        Text(String(format: "%02d", selected.stackIndex))
                            .font(.system(size: 13 * typeScale, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
                    }
                    .padding(.horizontal, 14).padding(.vertical, 12)
                    if let transform = model.selectedTransform {
                        sectionTitle("Transform", trailing: "At Playhead")
                            .padding(.horizontal, 14).padding(.top, 8).padding(.bottom, 4)
                        propertyControl("Position X", value: transform.positionX, unit: " px", path: "transform/position", component: 0, step: 10)
                        propertyControl("Position Y", value: transform.positionY, unit: " px", path: "transform/position", component: 1, step: 10)
                        propertyControl("Scale X", value: transform.scaleX, unit: "%", path: "transform/scale", component: 0, step: 5)
                        propertyControl("Scale Y", value: transform.scaleY, unit: "%", path: "transform/scale", component: 1, step: 5)
                        propertyControl("Rotation", value: transform.rotation, unit: "°", path: "transform/rotation", step: 5)
                        propertyControl("Opacity", value: transform.opacity, unit: "%", path: "transform/opacity", step: 5)
                    } else {
                        Text("Transform properties are unavailable for this layer.")
                            .font(.system(size: 12 * typeScale))
                            .foregroundStyle(StudioPalette.secondary)
                            .padding(.horizontal, 14).padding(.vertical, 8)
                    }
                    keyValue("IN", value: timecode(selected.inSeconds))
                    keyValue("OUT", value: timecode(selected.outSeconds))
                    keyValue("DURATION", value: String(format: "%.2f s", selected.durationSeconds))
                } else {
                    Text("Select a layer in the timeline to inspect it.")
                        .font(.system(size: 13 * typeScale)).foregroundStyle(StudioPalette.secondary)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                        .padding(14)
                }
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(StudioPalette.panel)
    }

    private var timelinePanel: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                sectionTitle("Timeline", trailing: "\(model.activeComposition?.layers.count ?? 0) Layers")
                Spacer()
                Button { showImportMedia = true } label: { Image(systemName: "square.and.arrow.down.on.square").font(.system(size: 14 * typeScale)).foregroundStyle(.white.opacity(0.66)) }
                    .frame(width: controlTargetSize, height: controlTargetSize)
                    .accessibilityLabel("Import media")
                Button { model.addSolid() } label: { Image(systemName: "plus").font(.system(size: 14 * typeScale, weight: .semibold)).foregroundStyle(StudioPalette.accent) }
                    .frame(width: controlTargetSize, height: controlTargetSize)
                    .accessibilityLabel("Add solid layer")
            }
            .padding(.horizontal, 14).frame(height: max(48, controlTargetSize + 4))
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            HStack(spacing: 0) {
                Text("Layer").frame(width: 184, alignment: .leading)
                HStack {
                    Text("00:00"); Spacer(); Text(timecode((model.activeComposition?.durationSeconds ?? 0) / 2)); Spacer(); Text(timecode(model.activeComposition?.durationSeconds ?? 0))
                }
            }
            .font(.system(size: 10 * typeScale, weight: .medium, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
            .frame(height: 36).padding(.horizontal, 12)
            Rectangle().fill(StudioPalette.line).frame(height: 1)
            ScrollView(.vertical, showsIndicators: false) {
                VStack(spacing: 0) {
                    ForEach(model.activeComposition?.layers ?? []) { layer in
                        timelineRow(layer)
                        Rectangle().fill(StudioPalette.line.opacity(0.7)).frame(height: 1)
                    }
                    if model.activeComposition?.layers.isEmpty ?? true {
                        Text("No layers yet · use + to add a solid")
                            .font(.system(size: 12 * typeScale)).foregroundStyle(StudioPalette.secondary)
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
                        .font(.system(size: 10 * typeScale, design: .monospaced)).foregroundStyle(StudioPalette.secondary)
                        .frame(width: 24, alignment: .leading)
                    Image(systemName: iconFor(layer.kind)).font(.system(size: 11 * typeScale)).foregroundStyle(isSelected ? StudioPalette.accent : .white.opacity(0.45))
                    Text(layer.name).font(.system(size: 12 * typeScale, weight: isSelected ? .semibold : .regular)).lineLimit(1)
                    Spacer(minLength: 0)
                }
                .frame(width: 184, alignment: .leading)
                GeometryReader { geometry in
                    let width = geometry.size.width
                    let left = max(0, min(width, width * layer.inSeconds / duration))
                    let right = max(left + 12, min(width, width * layer.outSeconds / duration))
                    ZStack(alignment: .leading) {
                        Rectangle().fill(Color.white.opacity(0.035))
                        RoundedRectangle(cornerRadius: 2)
                            .fill(isSelected ? StudioPalette.accent : Color.white.opacity(0.22))
                            .frame(width: max(12, right - left), height: 22)
                            .offset(x: left)
                        Rectangle().fill(Color.white.opacity(0.35)).frame(width: 1, height: 34)
                            .offset(x: max(0, min(width - 1, width * model.playheadSeconds / duration)))
                    }
                }
                .frame(height: 30)
            }
            .padding(.horizontal, 12).frame(minHeight: max(48, controlTargetSize + 4))
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
                Text("IMPORT MEDIA").font(.system(size: 10 * typeScale, weight: .semibold, design: .monospaced)).tracking(1)
                Spacer()
            }
            .foregroundStyle(.white.opacity(0.72))
            .padding(.horizontal, 10).padding(.vertical, 11)
            .frame(minHeight: controlTargetSize)
            .background(StudioPalette.raised)
        }
        .buttonStyle(.plain)
        .contentShape(Rectangle())
    }

    private func sectionTitle(_ title: String, trailing: String) -> some View {
        HStack {
            Text(title).foregroundStyle(.white.opacity(0.70))
            Spacer()
            Text(trailing).foregroundStyle(StudioPalette.secondary)
        }
        .font(.system(size: 12 * typeScale, weight: .semibold))
    }

    private func propertyControl(_ title: String, value: Double, unit: String = "", path: String, component: Int? = nil, step: Double) -> some View {
        HStack(spacing: 4) {
            Text(title)
                .font(.system(size: 11 * typeScale, weight: .medium))
                .foregroundStyle(StudioPalette.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
                .frame(width: 88, alignment: .leading)
            Button {
                model.adjustSelectedProperty(path: path, component: component, delta: -step)
            } label: {
                Image(systemName: "minus")
                    .font(.system(size: 13 * typeScale, weight: .semibold))
                    .frame(width: controlTargetSize, height: controlTargetSize)
            }
            .buttonStyle(ChromeButtonStyle())
            .accessibilityLabel("Decrease \(title)")
            Text("\(Int(value.rounded()))\(unit)")
                .font(.system(size: 12 * typeScale, weight: .medium, design: .monospaced))
                .foregroundStyle(.white.opacity(0.82))
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .frame(width: 68, alignment: .center)
            Button {
                model.adjustSelectedProperty(path: path, component: component, delta: step)
            } label: {
                Image(systemName: "plus")
                    .font(.system(size: 13 * typeScale, weight: .semibold))
                    .frame(width: controlTargetSize, height: controlTargetSize)
            }
            .buttonStyle(ChromeButtonStyle())
            .accessibilityLabel("Increase \(title)")
        }
        .padding(.horizontal, 14).padding(.vertical, 3)
    }

    private func keyValue(_ key: String, value: String) -> some View {
        HStack {
            Text(key).font(.system(size: 10 * typeScale, weight: .medium, design: .monospaced)).tracking(0.8).foregroundStyle(StudioPalette.secondary)
            Spacer()
            Text(value).font(.system(size: 11 * typeScale, design: .monospaced)).foregroundStyle(.white.opacity(0.78))
        }
        .padding(.horizontal, 14).padding(.vertical, 5)
    }

    private func toolButton(_ symbol: String, title: String, active: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 17 * typeScale, weight: .regular))
                .foregroundStyle(active ? StudioPalette.accent : .white.opacity(0.65))
                .frame(width: controlTargetSize, height: controlTargetSize)
                .background(active ? StudioPalette.accent.opacity(0.13) : .clear)
        }
        .buttonStyle(.plain)
        .contentShape(Rectangle())
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
        let safe = min(3_600_000, max(0, seconds.isFinite ? seconds : 0))
        let whole = Int(safe)
        let rate = model.activeComposition?.frameRate ?? 30
        let nominalRate = rate.isFinite ? min(240, max(1, rate.rounded())) : 30
        let frames = Int((safe - Double(whole)) * nominalRate)
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

    private func beginSaveProject() {
        guard let data = model.projectDocumentData() else {
            model.presentNotice("The project could not be serialized for saving.")
            return
        }
        exportDocument = ProjectDocument(data: data)
        showSaveProject = true
    }

    private func handleSaveCompletion(_ result: Result<URL, Error>) {
        switch result {
        case .success(let url): model.savedAs(url.lastPathComponent, savedData: exportDocument?.data)
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
