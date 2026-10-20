import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// The iOS host for the real egui workspace.  It deliberately contains no SwiftUI
/// recreation of the editor: every visible pixel comes from the Rust egui frame.
struct OriginalWorkspaceView: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> OriginalWorkspaceController {
        OriginalWorkspaceController()
    }

    func updateUIViewController(_ controller: OriginalWorkspaceController, context: Context) {}
}

final class OriginalWorkspaceController: UIViewController, UIDocumentPickerDelegate, UIScrollViewDelegate {
    private let viewport = UIScrollView()
    private let canvas = OriginalWorkspaceCanvas()
    private var lastViewportSize = CGSize.zero
    private var fitZoom: CGFloat = 1
    private var hasLaidOutViewport = false
    private var userZoomed = false
    private var applyingViewportLayout = false
    private var worker: OriginalWorkspaceWorker!
    private var dialogIDs = Set<UInt64>()
    private var dialogResponse: DialogResponse?
    private var activeFolderURL: URL?
    private var activeProjectURL: URL?
    private var projectDialogIDs = Set<UInt64>()
    private var pendingExport: (URL, String, Bool)?
    private var lastError: String?
    private var exportPoll: DispatchWorkItem?
    private var smokeFrames = 0
    private var smokeReady = false

    override func loadView() {
        viewport.backgroundColor = .black
        viewport.delegate = self
        viewport.panGestureRecognizer.minimumNumberOfTouches = 2
        viewport.panGestureRecognizer.maximumNumberOfTouches = 2
        viewport.addSubview(canvas)
        view = viewport
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        canvas.onEvent = { [weak self] event in self?.worker?.enqueue(event) }
        canvas.onNeedKeyboard = { [weak self] wants in
            guard let self else { return }
            if !self.canvas.isFirstResponder { _ = self.canvas.becomeFirstResponder() }
            self.canvas.wantsKeyboard = wants
            self.canvas.reloadInputViews()
        }
        worker = OriginalWorkspaceWorker { [weak self] result in
            DispatchQueue.main.async { self?.consume(result) }
        }
        NotificationCenter.default.addObserver(self, selector: #selector(pausePresentation), name: UIApplication.didEnterBackgroundNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(resumePresentation), name: UIApplication.willEnterForegroundNotification, object: nil)
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        layoutViewport(preservingZoom: hasLaidOutViewport && userZoomed)
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        layoutViewport(preservingZoom: hasLaidOutViewport && userZoomed)
        _ = canvas.becomeFirstResponder()
        worker.start()
        if ProcessInfo.processInfo.arguments.contains("--ui-smoke"), UIDevice.current.userInterfaceIdiom == .pad,
           let scene = view.window?.windowScene {
            if #available(iOS 16.0, *) {
                scene.requestGeometryUpdate(.iOS(interfaceOrientations: .landscape))
            }
        }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        worker.stop()
    }

    @objc private func pausePresentation() { worker?.stop() }
    @objc private func resumePresentation() { if viewIfLoaded?.window != nil { worker?.start() } }

    deinit {
        NotificationCenter.default.removeObserver(self)
        exportPoll?.cancel()
        activeFolderURL?.stopAccessingSecurityScopedResource()
        activeProjectURL?.stopAccessingSecurityScopedResource()
        worker?.stopAndDestroy()
    }

    private func consume(_ result: FrameResult) {
        if let image = result.image {
            canvas.image = image
            recordSmokeFrame(image)
        }
        if result.wantsKeyboard != canvas.wantsKeyboard {
            canvas.wantsKeyboard = result.wantsKeyboard
            canvas.onNeedKeyboard?(result.wantsKeyboard)
        }
        guard let output = result.output else { return }
        if let text = output.copiedText { UIPasteboard.general.string = text }
        if let urlString = output.openURL, let url = URL(string: urlString), UIApplication.shared.canOpenURL(url) {
            UIApplication.shared.open(url)
        }
        if let error = output.error, !error.isEmpty {
            recordSmokeStatus(["ok": false, "error": error], name: "workspace-error.json")
            showErrorOnce(error)
        }
        if let dialog = output.fileDialog, !dialogIDs.contains(dialog.id) {
            dialogIDs.insert(dialog.id)
            present(dialog: dialog)
        }
    }

    private func recordSmokeStatus(_ value: [String: Any], name: String) {
        guard ProcessInfo.processInfo.arguments.contains("--ui-smoke") else { return }
        let directory = applicationDocuments().appendingPathComponent("UISmoke", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
            try data.write(to: directory.appendingPathComponent(name), options: .atomic)
        } catch { NSLog("Unwritten Studio smoke evidence: %@", error.localizedDescription) }
    }

    private func recordSmokeFrame(_ image: UIImage) {
        guard ProcessInfo.processInfo.arguments.contains("--ui-smoke"), !smokeReady else { return }
        smokeFrames += 1
        guard smokeFrames >= 10, let cg = image.cgImage,
              let data = cg.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return }
        let count = CFDataGetLength(data)
        guard count >= 4 else { return }
        let hasVisibleContent = stride(from: 0, to: count - 3, by: 64).contains { index in
            bytes[index] > 80 || bytes[index + 1] > 80 || bytes[index + 2] > 80
        }
        guard hasVisibleContent else { return }
        smokeReady = true
        recordSmokeStatus(["ok": true, "renderedFrames": smokeFrames, "width": cg.width, "height": cg.height, "nonBlack": true], name: "workspace-ready.json")
    }

    private func layoutViewport(preservingZoom: Bool) {
        guard viewport.bounds.width > 0, viewport.bounds.height > 0 else { return }
        guard !hasLaidOutViewport || lastViewportSize != viewport.bounds.size else { return }
        lastViewportSize = viewport.bounds.size
        let oldFit = fitZoom
        let oldRelativeZoom = oldFit > 0 ? viewport.zoomScale / oldFit : 1
        let canvasSize = CGSize(width: max(1024, viewport.bounds.width), height: max(640, viewport.bounds.height))
        fitZoom = max(0.01, min(viewport.bounds.width / canvasSize.width, viewport.bounds.height / canvasSize.height, 1))
        applyingViewportLayout = true
        viewport.setZoomScale(1, animated: false)
        canvas.frame = CGRect(origin: .zero, size: canvasSize)
        viewport.contentSize = canvasSize
        viewport.minimumZoomScale = fitZoom
        viewport.maximumZoomScale = 2
        viewport.zoomScale = preservingZoom ? min(max(fitZoom * oldRelativeZoom, fitZoom), 2) : fitZoom
        updateViewportInsets()
        applyingViewportLayout = false
        hasLaidOutViewport = true
        updateGestureOwnership()
        worker?.setGeometry(canvas.bounds.size, scale: view.window?.screen.scale ?? UIScreen.main.scale)
    }

    private func updateViewportInsets() {
        let scaled = CGSize(width: canvas.bounds.width * viewport.zoomScale, height: canvas.bounds.height * viewport.zoomScale)
        let horizontal = max(0, (viewport.bounds.width - scaled.width) / 2)
        let vertical = max(0, (viewport.bounds.height - scaled.height) / 2)
        let insets = UIEdgeInsets(top: vertical, left: horizontal, bottom: vertical, right: horizontal)
        viewport.contentInset = insets
        viewport.scrollIndicatorInsets = insets
    }

    private func updateGestureOwnership() {
        let narrow = viewport.bounds.width < 1024
        let viewportPans = narrow || viewport.zoomScale > fitZoom + 0.001
        // Re-enabling resets a recognizer cleanly after a rotation or zoom
        // transition and avoids two independent two-finger scroll owners.
        viewport.panGestureRecognizer.isEnabled = false
        viewport.panGestureRecognizer.isEnabled = viewportPans
        canvas.internalScrollEnabled = !narrow && !viewportPans
    }

    func viewForZooming(in scrollView: UIScrollView) -> UIView? { canvas }

    func scrollViewDidZoom(_ scrollView: UIScrollView) {
        if !applyingViewportLayout { userZoomed = abs(scrollView.zoomScale - fitZoom) > 0.001 }
        updateViewportInsets()
        updateGestureOwnership()
    }

    private func showErrorOnce(_ message: String) {
        guard lastError != message else { return }
        lastError = message
        guard presentedViewController == nil else { return }
        let alert = UIAlertController(title: "Unwritten Studio", message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        present(alert, animated: true)
    }

    private func present(dialog: FileDialog) {
        if dialog.project { projectDialogIDs.insert(dialog.id) }
        switch dialog.kind {
        case "save": presentSave(dialog)
        case "folder": presentFolder(dialog)
        default: presentOpen(dialog)
        }
    }

    private func presentOpen(_ dialog: FileDialog) {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: dialog.contentTypes, asCopy: false)
        picker.allowsMultipleSelection = dialog.many
        picker.delegate = self
        picker.modalPresentationStyle = .formSheet
        picker.view.accessibilityIdentifier = "unwritten-file-dialog-\(dialog.id)"
        objc_setAssociatedObject(picker, &DialogAssociation.key, dialog.id, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        present(picker, animated: true)
    }

    private func presentFolder(_ dialog: FileDialog) {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder], asCopy: false)
        picker.allowsMultipleSelection = false
        picker.delegate = self
        picker.modalPresentationStyle = .formSheet
        objc_setAssociatedObject(picker, &DialogAssociation.key, dialog.id, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        present(picker, animated: true)
    }

    private func presentSave(_ dialog: FileDialog) {
        let alert = UIAlertController(title: "Save Project", message: nil, preferredStyle: .alert)
        alert.addTextField { field in
            field.text = dialog.defaultName ?? "Untitled.uwproj"
            field.placeholder = "File name"
            field.clearButtonMode = .whileEditing
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { [weak self] _ in self?.finishDialog(dialog.id, paths: []) })
        alert.addAction(UIAlertAction(title: "Save", style: .default) { [weak self, weak alert] _ in
            guard let self, let name = alert?.textFields?.first?.text?.trimmingCharacters(in: .whitespacesAndNewlines), !name.isEmpty else {
                self?.finishDialog(dialog.id, paths: []); return
            }
            var safeName = self.safeFileName(name)
            if URL(fileURLWithPath: safeName).pathExtension.isEmpty, let ext = dialog.extensions.first, !ext.isEmpty { safeName += "." + ext }
            let exportDirectory = self.applicationDocuments()
                .appendingPathComponent("Exports", isDirectory: true)
                .appendingPathComponent(UUID().uuidString, isDirectory: true)
            do {
                try FileManager.default.createDirectory(at: exportDirectory, withIntermediateDirectories: true)
            } catch {
                self.showErrorOnce("Could not prepare the export location: \(error.localizedDescription)")
                self.finishDialog(dialog.id, paths: [])
                return
            }
            let destination = exportDirectory.appendingPathComponent(safeName, isDirectory: false)
            self.pendingExport = (destination, safeName, dialog.project)
            self.finishDialog(dialog.id, paths: [destination.path])
            self.waitForSaveFile(destination, name: safeName, project: dialog.project)
        })
        present(alert, animated: true)
    }

    private func applicationDocuments() -> URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first ?? URL(fileURLWithPath: NSTemporaryDirectory())
    }

    private func waitForSaveFile(_ url: URL, name: String, project: Bool) {
        exportPoll?.cancel()
        let deadline = Date().addingTimeInterval(120)
        var poll: DispatchWorkItem!
        poll = DispatchWorkItem { [weak self] in
            guard let self, !poll.isCancelled else { return }
            let exists = FileManager.default.fileExists(atPath: url.path)
            if exists || Date() >= deadline {
                DispatchQueue.main.async {
                    guard self.pendingExport?.0 == url else { return }
                    self.pendingExport = nil
                    if exists {
                        if project { self.presentExport(url: url, name: name) }
                    } else {
                        self.showErrorOnce("The engine did not finish writing \(name) within 120 seconds.")
                    }
                }
            } else {
                DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + 0.25, execute: poll)
            }
        }
        exportPoll = poll
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + 0.25, execute: poll)
    }

    private func safeFileName(_ value: String) -> String {
        let name = URL(fileURLWithPath: value).lastPathComponent
            .replacingOccurrences(of: "/", with: "-")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return name.isEmpty ? "Untitled.uwproj" : name
    }

    private func presentExport(url: URL, name: String) {
        pendingExport = nil
        let picker = UIDocumentPickerViewController(forExporting: [url], asCopy: true)
        picker.delegate = self
        picker.modalPresentationStyle = .formSheet
        picker.view.accessibilityLabel = name
        present(picker, animated: true)
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        if let id = objc_getAssociatedObject(controller, &DialogAssociation.key) as? UInt64 {
            projectDialogIDs.remove(id)
            finishDialog(id, paths: [])
        }
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let id = objc_getAssociatedObject(controller, &DialogAssociation.key) as? UInt64 else {
            // Export pickers have no dialog response and their URLs are not folder selections.
            return
        }
        if projectDialogIDs.remove(id) != nil, let projectURL = urls.first {
            activeProjectURL?.stopAccessingSecurityScopedResource()
            activeProjectURL = projectURL
            _ = projectURL.startAccessingSecurityScopedResource()
            // Keep the security-scoped source URL so relative media remains resolvable for this session.
            finishDialog(id, paths: [projectURL.path])
            return
        }
        let isFolder = urls.first?.hasDirectoryPath == true
        if isFolder, let folder = urls.first {
            activeFolderURL?.stopAccessingSecurityScopedResource()
            activeFolderURL = folder
            _ = folder.startAccessingSecurityScopedResource()
            finishDialog(id, paths: [folder.path])
        } else {
            finishDialog(id, paths: copyToImportedMedia(urls))
        }
    }

    private func copyToImportedMedia(_ urls: [URL]) -> [String] {
        let root = applicationDocuments().appendingPathComponent("Imported Media", isDirectory: true)
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        var failures: [String] = []
        let paths = urls.compactMap { source -> String? in
            let access = source.startAccessingSecurityScopedResource()
            defer { if access { source.stopAccessingSecurityScopedResource() } }
            let destination = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
                .appendingPathComponent(source.lastPathComponent, isDirectory: false)
            do {
                try FileManager.default.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
                try FileManager.default.copyItem(at: source, to: destination)
                return destination.path
            } catch {
                failures.append("\(source.lastPathComponent): \(error.localizedDescription)")
                return nil
            }
        }
        if !failures.isEmpty { showErrorOnce("Some media could not be imported: " + failures.joined(separator: "; ")) }
        return paths
    }

    private func finishDialog(_ id: UInt64, paths: [String]) {
        dialogResponse = DialogResponse(id: id, paths: paths)
        worker.setDialogResponse(dialogResponse)
        worker.enqueue(.wake)
    }
}

private enum DialogAssociation { static var key: UInt8 = 0 }

private final class OriginalWorkspaceWorker {
    private let queue = DispatchQueue(label: "org.unwritten.studio.egui", qos: .userInitiated)
    private var handle: OpaquePointer?
    private var timer: DispatchSourceTimer?
    private var running = false
    private var frameInFlight = false
    private var events: [InputEvent] = []
    private var logicalSize = CGSize(width: 320, height: 240)
    private var screenScale: CGFloat = 1
    private let deliver: (FrameResult) -> Void

    init(deliver: @escaping (FrameResult) -> Void) { self.deliver = deliver }
    func setGeometry(_ size: CGSize, scale: CGFloat) {
        queue.async { [weak self] in
            guard let self else { return }
            self.logicalSize = CGSize(width: max(1, size.width), height: max(1, size.height))
            self.screenScale = min(max(1, scale), 2)
        }
    }

    func start() {
        queue.async { [weak self] in
            guard let self, !self.running else { return }
            if self.handle == nil { self.handle = unwritten_studio_ui_create() }
            self.running = self.handle != nil
            guard self.running else {
                var reason = "The original workspace could not start."
                if let message = unwritten_studio_ui_last_error() {
                    reason += " " + String(cString: message)
                    unwritten_studio_string_free(message)
                }
                self.deliver(FrameResult(image: nil, output: UIOutput(ok: false, error: reason, wantsKeyboard: false, copiedText: nil, openURL: nil, fileDialog: nil), wantsKeyboard: false))
                return
            }
            let timer = DispatchSource.makeTimerSource(queue: self.queue)
            timer.schedule(deadline: .now(), repeating: .milliseconds(34), leeway: .milliseconds(4))
            timer.setEventHandler { [weak self] in autoreleasepool { self?.frame() } }
            self.timer = timer
            timer.resume()
        }
    }

    func stop() { queue.async { [weak self] in
        self?.timer?.suspendIfNeeded()
        self?.timer = nil
        self?.running = false
    } }

    func stopAndDestroy() {
        queue.sync {
            running = false
            timer?.cancel(); timer = nil
            if let handle { unwritten_studio_ui_destroy(handle); self.handle = nil }
        }
    }

    func enqueue(_ event: InputEvent) { queue.async { [weak self] in
        guard let self, self.events.count < 1024 else { return }
        self.events.append(event)
    } }
    func setDialogResponse(_ response: DialogResponse?) { queue.async { [weak self] in self?.pendingResponse = response } }

    private func frame() {
        guard running, !frameInFlight, let handle else { return }
        frameInFlight = true
        // Geometry is captured on the main thread and consumed here on the worker.
        let size = logicalSize
        var payload: [String: Any] = ["width": max(1, Int(size.width)), "height": max(1, Int(size.height)), "scale": screenScale, "time": ProcessInfo.processInfo.systemUptime, "events": events.map(\.json), "smoke": ProcessInfo.processInfo.arguments.contains("--ui-smoke")]
        events.removeAll(keepingCapacity: true)
        if let response = pendingResponse { payload["dialog"] = response.json; pendingResponse = nil }
        let jsonData = (try? JSONSerialization.data(withJSONObject: payload)) ?? Data("{}".utf8)
        let json = String(decoding: jsonData, as: UTF8.self)
        var width: UInt32 = 0; var height: UInt32 = 0
        var result = FrameResult(image: nil, output: nil, wantsKeyboard: false)
        json.withCString { input in
            guard let pixels = unwritten_studio_ui_frame(handle, input, &width, &height), width > 0, height > 0 else {
                let output = readOutput(handle)
                result = FrameResult(image: nil, output: output, wantsKeyboard: output?.wantsKeyboard ?? false)
                return
            }
            let w = UInt64(width), h = UInt64(height)
            let area = w.multipliedReportingOverflow(by: h)
            let byteCount = area.partialValue.multipliedReportingOverflow(by: 4)
            guard !area.overflow, !byteCount.overflow, byteCount.partialValue <= UInt64(Int.max) else {
                // The dimensions are invalid for a Swift allocation; do not attempt one.
                // An invalid length must never be passed to Rust's boxed-slice deallocator.
                running = false
                let output = readOutput(handle)
                result = FrameResult(image: nil, output: output, wantsKeyboard: output?.wantsKeyboard ?? false)
                return
            }
            let count = Int(byteCount.partialValue)
            defer { unwritten_studio_bytes_free(pixels, count) }
            guard width <= 16384, height <= 16384, count <= 512 * 1024 * 1024 else {
                let output = readOutput(handle)
                result = FrameResult(image: nil, output: output, wantsKeyboard: output?.wantsKeyboard ?? false)
                return
            }
            let data = Data(bytes: pixels, count: count)
            let output = readOutput(handle)
            result = FrameResult(image: makeImage(data: data, width: Int(width), height: Int(height)), output: output, wantsKeyboard: output?.wantsKeyboard ?? false)
        }
        frameInFlight = false
        deliver(result)
    }

    // Set by the controller through the next wake event; kept private to the serial worker.
    private var pendingResponse: DialogResponse?
    private func readOutput(_ handle: OpaquePointer) -> UIOutput? {
        guard let ptr = unwritten_studio_ui_output_json(handle) else { return nil }
        defer { unwritten_studio_string_free(ptr) }
        let text = String(cString: ptr)
        guard let data = text.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(UIOutput.self, from: data)
    }
}

private func makeImage(data: Data, width: Int, height: Int) -> UIImage? {
    guard let provider = CGDataProvider(data: data as CFData), let cg = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue), provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent) else { return nil }
    return UIImage(cgImage: cg)
}

private struct FrameResult { let image: UIImage?; let output: UIOutput?; let wantsKeyboard: Bool }
private struct DialogResponse { let id: UInt64; let paths: [String]; var json: [String: Any] { ["id": id, "paths": paths] } }
private struct FileDialog: Decodable { let id: UInt64; let kind: String; let extensions: [String]; let many: Bool; let defaultName: String?; let project: Bool
    var contentTypes: [UTType] { let types = extensions.compactMap { UTType(filenameExtension: $0) }; return types.isEmpty ? [.data] : types }
}
private struct UIOutput: Decodable { let ok: Bool?; let error: String?; let wantsKeyboard: Bool?; let copiedText: String?; let openURL: String?; let fileDialog: FileDialog? }

private enum InputEvent {
    case move(CGPoint), button(CGPoint, Bool), scroll(CGPoint, CGPoint), text(String), key(String, Bool, UIKeyModifierFlags), wake
    var json: [String: Any] {
        switch self {
        case let .move(p): return ["type":"move", "x":p.x, "y":p.y]
        case let .button(p, pressed): return ["type":"button", "x":p.x, "y":p.y, "pressed":pressed]
        case let .scroll(p, delta): return ["type":"scroll", "x":p.x, "y":p.y, "dx":delta.x, "dy":delta.y]
        case let .text(value): return ["type":"text", "text":value]
        case let .key(key, pressed, modifiers): return ["type":"key", "key":key, "pressed":pressed, "command":modifiers.contains(.command), "shift":modifiers.contains(.shift), "alt":modifiers.contains(.alternate), "ctrl":modifiers.contains(.control)]
        case .wake: return ["type":"wake"]
        }
    }
}

private final class OriginalWorkspaceCanvas: UIView, UIKeyInput {
    var image: UIImage? { didSet { imageView.image = image } }
    var wantsKeyboard = false
    var onEvent: ((InputEvent) -> Void)?
    var onNeedKeyboard: ((Bool) -> Void)?
    var internalScrollEnabled = true {
        didSet { internalPan?.isEnabled = internalScrollEnabled }
    }
    private let imageView = UIImageView()
    private var primaryTouch: UITouch?
    private var scrolling = false
    private let hiddenInputView = UIView(frame: .zero)
    private var internalPan: UIPanGestureRecognizer?

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        imageView.contentMode = .scaleAspectFit
        imageView.isUserInteractionEnabled = false
        addSubview(imageView)
        let pan = UIPanGestureRecognizer(target: self, action: #selector(pan(_:)))
        pan.minimumNumberOfTouches = 2
        pan.maximumNumberOfTouches = 2
        pan.cancelsTouchesInView = false
        addGestureRecognizer(pan)
        internalPan = pan
        isMultipleTouchEnabled = true
    }
    required init?(coder: NSCoder) { nil }
    override func layoutSubviews() { super.layoutSubviews(); imageView.frame = bounds }
    override var canBecomeFirstResponder: Bool { true }
    override var inputView: UIView? { wantsKeyboard ? nil : hiddenInputView }
    var hasText: Bool { true }
    func insertText(_ text: String) { onEvent?(.text(text)) }
    func deleteBackward() { onEvent?(.key("Backspace", true, [])); onEvent?(.key("Backspace", false, [])) }
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        for press in presses { if let key = press.key, let name = keyName(key) { onEvent?(.key(name, true, key.modifierFlags)) } }
        super.pressesBegan(presses, with: event)
    }
    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        for press in presses { if let key = press.key, let name = keyName(key) { onEvent?(.key(name, false, key.modifierFlags)) } }
        super.pressesEnded(presses, with: event)
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard !scrolling, primaryTouch == nil, let touch = touches.first else { return }
        primaryTouch = touch
        let p = touch.location(in: self)
        onEvent?(.move(p)); onEvent?(.button(p, true))
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard !scrolling, let touch = primaryTouch, touches.contains(touch) else { return }
        onEvent?(.move(touch.location(in: self)))
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let primary = primaryTouch, touches.contains(primary) { finishTouch(primary) }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let primary = primaryTouch, touches.contains(primary) { finishTouch(primary) }
    }
    private func finishTouch(_ touch: UITouch?) {
        guard !scrolling, let primary = primaryTouch, touch === primary else { return }
        onEvent?(.button(primary.location(in: self), false)); primaryTouch = nil
    }
    @objc private func pan(_ gesture: UIPanGestureRecognizer) {
        switch gesture.state {
        case .began:
            scrolling = true
            if let primary = primaryTouch { onEvent?(.button(primary.location(in: self), false)); primaryTouch = nil }
        case .changed:
            let p = gesture.location(in: self), d = gesture.translation(in: self)
            gesture.setTranslation(.zero, in: self); onEvent?(.scroll(p, d))
        case .ended, .cancelled, .failed:
            scrolling = false
        default: break
        }
    }
    private func keyName(_ key: UIKey) -> String? {
        switch key.keyCode {
        case .keyboardDeleteOrBackspace: return "Backspace"
        case .keyboardReturnOrEnter: return "Enter"
        case .keyboardEscape: return "Escape"
        case .keyboardTab: return "Tab"
        case .keyboardLeftArrow: return "ArrowLeft"
        case .keyboardRightArrow: return "ArrowRight"
        case .keyboardUpArrow: return "ArrowUp"
        case .keyboardDownArrow: return "ArrowDown"
        default: break
        }
        let chars = key.charactersIgnoringModifiers
        return chars.isEmpty ? nil : (chars.count == 1 ? chars.uppercased() : chars)
    }
}

private extension DispatchSourceTimer {
    func suspendIfNeeded() { /* cancellation is the safe lifecycle operation; kept idempotent by the worker */ cancel() }
}
