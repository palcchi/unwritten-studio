import AVFAudio
import Foundation

/// Owns output on the editor's serial worker queue. The real-time callback only
/// sees the independent Rust audio handle, never the thread-confined editor.
final class OriginalWorkspaceAudio {
    static let sampleRate = 48_000.0
    private var engine: AVAudioEngine?
    private var source: AVAudioSourceNode?
    private var audioHandle: OpaquePointer?

    var isRunning: Bool { engine?.isRunning == true }

    enum AudioError: LocalizedError {
        case unavailable(String)
        var errorDescription: String? {
            switch self { case let .unavailable(message): return message }
        }
    }

    init(handle: OpaquePointer?) throws {
        guard let handle else { throw AudioError.unavailable("Audio preview handle is unavailable.") }
        audioHandle = handle
    }

    func start() throws {
        guard !isRunning else { return }
        guard let handle = audioHandle else { throw AudioError.unavailable("Audio preview handle has been released.") }
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playback, mode: .default, options: [])
            try session.setPreferredSampleRate(Self.sampleRate)
            try session.setActive(true)
            guard let format = AVAudioFormat(standardFormatWithSampleRate: Self.sampleRate, channels: 2) else {
                throw AudioError.unavailable("Unable to create the stereo preview format.")
            }
            // Explicit non-interleaved Float32 matches AVAudioEngine's normal
            // connection format. Rust deinterleaves using a bounded stack buffer.
            let node = AVAudioSourceNode(format: format) { _, _, frames, list in
                let buffers = UnsafeMutableAudioBufferListPointer(list)
                for buffer in buffers {
                    if let data = buffer.mData { memset(data, 0, Int(buffer.mDataByteSize)) }
                }
                guard buffers.count == 2,
                      let left = buffers[0].mData?.assumingMemoryBound(to: Float.self),
                      let right = buffers[1].mData?.assumingMemoryBound(to: Float.self) else { return noErr }
                let capacity = min(Int(buffers[0].mDataByteSize), Int(buffers[1].mDataByteSize)) / MemoryLayout<Float>.stride
                let count = min(Int(frames), capacity)
                _ = unwritten_studio_audio_pull_planar(handle, left, right, count)
                return noErr
            }
            let audioEngine = AVAudioEngine()
            audioEngine.attach(node)
            audioEngine.connect(node, to: audioEngine.mainMixerNode, format: format)
            do {
                try audioEngine.start()
            } catch {
                audioEngine.stop()
                audioEngine.detach(node)
                throw error
            }
            source = node
            engine = audioEngine
        } catch {
            try? session.setActive(false, options: .notifyOthersOnDeactivation)
            throw AudioError.unavailable("Audio preview could not start: \(error.localizedDescription)")
        }
    }

    func stop() {
        // Stop callbacks before detaching nodes or releasing the Rust handle.
        engine?.stop()
        if let source { engine?.detach(source) }
        source = nil
        engine = nil
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    func destroy() {
        stop()
        if let handle = audioHandle {
            audioHandle = nil
            unwritten_studio_audio_destroy(handle)
        }
    }

    deinit { destroy() }
}
