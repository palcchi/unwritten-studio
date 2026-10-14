import Combine
import CoreGraphics
import Foundation
import SwiftUI
import UIKit

struct StudioItem: Decodable, Identifiable {
    let id: UInt64
    let name: String
    let kind: String
    let width: Int?
    let height: Int?
    let durationSeconds: Double?
    let layerCount: Int?
}

struct StudioLayer: Decodable, Identifiable {
    let id: UInt64
    let name: String
    let kind: String
    let stackIndex: Int
    let inSeconds: Double
    let outSeconds: Double
    let startSeconds: Double
    let durationSeconds: Double
}

struct StudioLayerTransform {
    let position: [Double]
    let scale: [Double]
    let rotation: Double
    let opacity: Double

    var positionX: Double { position[0] }
    var positionY: Double { position[1] }
    var scaleX: Double { scale[0] }
    var scaleY: Double { scale[1] }
}

struct StudioComposition: Decodable, Identifiable {
    let id: UInt64
    let name: String
    let width: Int
    let height: Int
    let durationSeconds: Double
    let frameRate: Double
    let layers: [StudioLayer]
}

struct StudioSnapshot: Decodable {
    let projectName: String
    let hasProject: Bool
    let items: [StudioItem]
    let activeComp: StudioComposition?
    let selectedLayers: [UInt64]
    let currentTimeSeconds: Double
    let dirty: Bool
    let canUndo: Bool
    let canRedo: Bool
    let renderer: String
    let rendererNote: String?
}

private struct EngineReply: Decodable {
    let ok: Bool
    let result: JSONValue?
    let error: String?
}

private enum JSONValue: Decodable {
    case object([String: JSONValue])
    case array([JSONValue])
    case string(String)
    case number(Double)
    case bool(Bool)
    case null

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if let value = try? container.decode(Bool.self) { self = .bool(value) }
        else if let value = try? container.decode(Double.self) { self = .number(value) }
        else if let value = try? container.decode(String.self) { self = .string(value) }
        else if let value = try? container.decode([JSONValue].self) { self = .array(value) }
        else { self = .object(try container.decode([String: JSONValue].self)) }
    }
}

private extension JSONValue {
    var number: Double? {
        guard case let .number(value) = self else { return nil }
        return value
    }

    var numbers: [Double]? {
        guard case let .array(values) = self else { return nil }
        let numbers = values.compactMap { $0.number }
        return numbers.count == values.count ? numbers : nil
    }
}

private struct RenderedFrame: Sendable {
    let width: Int
    let height: Int
    let pixels: Data

    @MainActor
    func image() -> UIImage? {
        guard width > 0, height > 0,
              let provider = CGDataProvider(data: pixels as CFData),
              let colorSpace = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
        let info = CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue)
        guard let cgImage = CGImage(width: width, height: height, bitsPerComponent: 8,
                                    bitsPerPixel: 32, bytesPerRow: width * 4,
                                    space: colorSpace, bitmapInfo: info,
                                    provider: provider, decode: nil,
                                    shouldInterpolate: true, intent: .defaultIntent) else { return nil }
        return UIImage(cgImage: cgImage)
    }
}

private final class RustCore: @unchecked Sendable {
    private let handle: OpaquePointer

    init?() {
        guard let handle = unwritten_studio_create() else { return nil }
        self.handle = handle
    }

    deinit {
        unwritten_studio_destroy(handle)
    }

    private func consume(_ pointer: UnsafeMutablePointer<CChar>?) -> String? {
        guard let pointer else { return nil }
        defer { unwritten_studio_string_free(pointer) }
        return String(cString: pointer)
    }

    func snapshotJSON() -> String? {
        consume(unwritten_studio_snapshot_json(handle))
    }

    func execute(_ command: String, parameters: [String: Any] = [:]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: parameters, options: [.sortedKeys]),
              let json = String(data: data, encoding: .utf8) else { return nil }
        return command.withCString { commandPointer in
            json.withCString { jsonPointer in
                consume(unwritten_studio_execute(handle, commandPointer, jsonPointer))
            }
        }
    }

    func newProject(_ name: String) -> String? {
        name.withCString { consume(unwritten_studio_new_project(handle, $0)) }
    }

    func loadProject(_ json: String) -> String? {
        json.withCString { consume(unwritten_studio_load_project_json(handle, $0)) }
    }

    func serializeProject() -> String? {
        consume(unwritten_studio_serialize_project_json(handle))
    }

    func importMedia(at path: String) -> String? {
        path.withCString { consume(unwritten_studio_import_media(handle, $0)) }
    }

    func setTime(_ seconds: Double) -> String? {
        consume(unwritten_studio_set_time(handle, seconds))
    }

    func undo() -> String? { consume(unwritten_studio_undo(handle)) }
    func redo() -> String? { consume(unwritten_studio_redo(handle)) }
    func markSaved() { unwritten_studio_mark_saved(handle) }

    func render(maxSide: UInt32) -> RenderedFrame? {
        var width: UInt32 = 0
        var height: UInt32 = 0
        guard let bytes = unwritten_studio_render_rgba8(handle, maxSide, &width, &height) else { return nil }
        let length = Int(width) * Int(height) * 4
        defer { unwritten_studio_bytes_free(bytes, length) }
        return RenderedFrame(width: Int(width), height: Int(height), pixels: Data(bytes: bytes, count: length))
    }
}

@MainActor
final class StudioModel: ObservableObject {
    @Published private(set) var snapshot: StudioSnapshot?
    @Published private(set) var preview: UIImage?
    @Published private(set) var isReady = false
    @Published private(set) var isRendering = false
    @Published private(set) var isImporting = false
    @Published private(set) var notice: String?
    @Published private(set) var documentName = "Untitled Project"
    @Published private(set) var selectedTransform: StudioLayerTransform?
    @Published var playheadSeconds: Double = 0

    private var core: RustCore?
    private var renderTask: Task<Void, Never>?

    init() {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let newCore = RustCore()
            DispatchQueue.main.async {
                guard let self else { return }
                guard let newCore else {
                    self.notice = "The local editor engine could not start."
                    return
                }
                self.core = newCore
                self.isReady = true
                self.refreshSnapshot()
            }
        }
    }

    var activeComposition: StudioComposition? { snapshot?.activeComp }
    var hasProject: Bool { snapshot?.hasProject ?? false }
    var isDirty: Bool { snapshot?.dirty ?? false }

    func createProject(name: String = "Untitled") {
        guard let core else { return }
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let title = trimmed.isEmpty ? "Untitled" : trimmed
        runReply(core.newProject(title))
        documentName = title
        refreshSnapshot(render: true)
    }

    func openProject(data: Data, displayName: String) {
        guard let core, let text = String(data: data, encoding: .utf8) else {
            notice = "This project file is not valid UTF-8 JSON."
            return
        }
        guard runReply(core.loadProject(text)) else { return }
        documentName = displayName
        refreshSnapshot(render: true)
    }

    func projectDocumentData() -> Data? {
        guard let text = core?.serializeProject() else { return nil }
        return Data(text.utf8)
    }

    func savedAs(_ filename: String) {
        core?.markSaved()
        documentName = URL(fileURLWithPath: filename).deletingPathExtension().lastPathComponent
        refreshSnapshot()
    }

    func addSolid() {
        perform("layer.newSolid", parameters: ["name": "Color Matte", "color": "#E85D45"], render: true)
    }

    func addShape() {
        perform("layer.newShape", parameters: ["name": "Rectangle", "kind": "rect", "size": [540, 320], "fill": "#E85D45", "position": [960, 540]], render: true)
    }

    func addText() {
        perform("layer.newText", parameters: ["name": "Title", "text": "Unwritten", "font": "Inter", "size": 112, "fill": "#F2F1EE", "position": [960, 540]], render: true)
    }

    func importMedia(from source: URL) {
        guard let core else { return }
        isImporting = true
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let didAccess = source.startAccessingSecurityScopedResource()
            defer { if didAccess { source.stopAccessingSecurityScopedResource() } }
            do {
                let support = try FileManager.default.url(for: .applicationSupportDirectory,
                                                          in: .userDomainMask, appropriateFor: nil, create: true)
                    .appendingPathComponent("Unwritten Studio/Imported Media", isDirectory: true)
                try FileManager.default.createDirectory(at: support, withIntermediateDirectories: true)
                let safeName = source.lastPathComponent.isEmpty ? "Imported Media" : source.lastPathComponent
                var destination = support.appendingPathComponent(safeName)
                if FileManager.default.fileExists(atPath: destination.path) {
                    let stem = destination.deletingPathExtension().lastPathComponent
                    let suffix = destination.pathExtension
                    destination = support.appendingPathComponent("\(stem)-\(UUID().uuidString.prefix(8))")
                    if !suffix.isEmpty { destination.appendPathExtension(suffix) }
                }
                try FileManager.default.copyItem(at: source, to: destination)
                let reply = core.importMedia(at: destination.path)
                DispatchQueue.main.async {
                    guard let self else { return }
                    self.isImporting = false
                    if !self.runReply(reply) { return }
                    self.refreshSnapshot(render: true)
                }
            } catch {
                DispatchQueue.main.async {
                    self?.isImporting = false
                    self?.notice = "Could not copy the selected media: \(error.localizedDescription)"
                }
            }
        }
    }

    func openComposition(_ id: UInt64) {
        perform("comp.open", parameters: ["comp": id], render: true)
    }

    func selectLayer(_ id: UInt64) {
        perform("layer.select", parameters: ["layers": [id]])
    }

    func seek(to seconds: Double, render: Bool = false) {
        guard let core else { return }
        _ = core.setTime(seconds)
        playheadSeconds = snapshot?.activeComp.map { min(max(0, seconds), $0.durationSeconds) } ?? 0
        if render {
            refreshSnapshot()
            refreshPreview()
        }
    }

    func undo() {
        guard let core else { return }
        _ = core.undo()
        refreshSnapshot(render: true)
    }

    func redo() {
        guard let core else { return }
        _ = core.redo()
        refreshSnapshot(render: true)
    }

    func refreshSnapshot(render: Bool = false) {
        guard let core, let raw = core.snapshotJSON(), let data = raw.data(using: .utf8) else { return }
        do {
            snapshot = try JSONDecoder().decode(StudioSnapshot.self, from: data)
            if let duration = snapshot?.activeComp?.durationSeconds {
                playheadSeconds = min(max(0, playheadSeconds), duration)
            }
            refreshSelectedTransform()
            if render { refreshPreview() }
        } catch {
            notice = "The editor engine returned an unreadable project snapshot."
        }
    }

    func refreshPreview() {
        guard let core, activeComposition != nil else { preview = nil; return }
        renderTask?.cancel()
        isRendering = true
        renderTask = Task.detached(priority: .userInitiated) { [weak self] in
            let frame = core.render(maxSide: 1280)
            await MainActor.run {
                guard let self else { return }
                if let frame { self.preview = frame.image() }
                self.isRendering = false
            }
        }
    }

    func adjustSelectedProperty(path: String, component: Int? = nil, delta: Double) {
        guard let core, let layerID = snapshot?.selectedLayers.first,
              delta.isFinite, delta != 0,
              let current = propertyValue(layerID: layerID, path: path) else { return }

        let nextValue: Any
        if let component {
            guard var values = current.numbers, values.indices.contains(component) else { return }
            values[component] += delta
            nextValue = values
        } else {
            guard var value = current.number else { return }
            value += delta
            if path == "transform/opacity" { value = min(100, max(0, value)) }
            nextValue = value
        }

        guard runReply(core.execute("prop.set", parameters: [
            "layer": layerID,
            "path": path,
            "value": nextValue,
        ])) else { return }
        refreshSnapshot(render: true)
    }

    private func refreshSelectedTransform() {
        guard let layerID = snapshot?.selectedLayers.first,
              let position = propertyValue(layerID: layerID, path: "transform/position")?.numbers,
              position.count >= 2,
              let scale = propertyValue(layerID: layerID, path: "transform/scale")?.numbers,
              scale.count >= 2,
              let rotation = propertyValue(layerID: layerID, path: "transform/rotation")?.number,
              let opacity = propertyValue(layerID: layerID, path: "transform/opacity")?.number else {
            selectedTransform = nil
            return
        }
        selectedTransform = StudioLayerTransform(position: position, scale: scale, rotation: rotation, opacity: opacity)
    }

    private func propertyValue(layerID: UInt64, path: String) -> JSONValue? {
        guard let raw = core?.execute("prop.get", parameters: ["layer": layerID, "path": path]),
              let data = raw.data(using: .utf8),
              let reply = try? JSONDecoder().decode(EngineReply.self, from: data),
              reply.ok,
              case let .object(fields)? = reply.result else { return nil }
        return fields["value"]
    }

    func dismissNotice() { notice = nil }

    func presentNotice(_ message: String) { notice = message }

    private func perform(_ command: String, parameters: [String: Any] = [:], render: Bool = false) {
        guard let core else { return }
        guard runReply(core.execute(command, parameters: parameters)) else { return }
        refreshSnapshot(render: render)
    }

    @discardableResult
    private func runReply(_ raw: String?) -> Bool {
        guard let raw, let data = raw.data(using: .utf8),
              let reply = try? JSONDecoder().decode(EngineReply.self, from: data) else {
            notice = "The local editor engine returned no response."
            return false
        }
        guard reply.ok else {
            notice = reply.error ?? "The requested edit could not be applied."
            return false
        }
        return true
    }
}
