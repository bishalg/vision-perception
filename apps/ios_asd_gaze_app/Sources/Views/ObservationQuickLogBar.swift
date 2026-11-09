import AVFoundation
import PhotosUI
import Speech
import SwiftUI

/// S-15: dictation and photo attachments appear only while their opt-ins are on.
public struct ObservationQuickLogBar: View {
    @ObservedObject var store: ObservationLogStore
    @ObservedObject private var optIns = FeatureOptIns.shared
    @State private var isRecording = false
    @State private var speechError: String?
    @State private var pickerItem: PhotosPickerItem?
    @StateObject private var dictation = ObservationDictationController()

    public init(store: ObservationLogStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Today's observation")
                    .font(.headline)
                Spacer()
                ClinicalTermTooltip(term: ClinicalGlossary.jointAttention)
            }

            HStack(alignment: .top, spacing: 8) {
                TextField("What did you notice?", text: $store.today.notes, axis: .vertical)
                    .lineLimit(2...5)
                if optIns.dictation {
                    Button {
                        Task { await toggleDictation() }
                    } label: {
                        Image(systemName: isRecording ? "mic.fill" : "mic")
                            .foregroundStyle(isRecording ? LiquidGlass.signalAtypical : LiquidGlass.celestialTeal)
                    }
                    .accessibilityLabel("Speech to text")
                }
            }

            HStack {
                ForEach(BehavioralObservationTag.allCases) { tag in
                    let on = store.today.tags.contains(tag)
                    Button {
                        store.toggle(tag: tag)
                    } label: {
                        Label(tag.rawValue, systemImage: tag.systemImage)
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 6)
                            .background(on ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.08), in: Capsule())
                    }
                    .buttonStyle(.plain)
                }
            }

            if optIns.photoAttachments {
                PhotosPicker(selection: $pickerItem, matching: .images) {
                    Label(
                        store.today.hasOnDeviceMedia ? "Photo attached on device" : "Attach a photo (stays on device)",
                        systemImage: "photo.on.rectangle"
                    )
                    .font(.caption)
                }
                .onChange(of: pickerItem) { _, item in
                    store.today.hasOnDeviceMedia = item != nil
                }
            }

            if !optIns.dictation || !optIns.photoAttachments {
                Text(Localizer.tr("quicklog_optins_off"))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }

            if let speechError {
                Text(speechError).font(.caption2).foregroundStyle(.secondary)
            }
        }
    }

    private func toggleDictation() async {
        if isRecording {
            dictation.stop()
            isRecording = false
            return
        }
        do {
            try await dictation.start { text in
                if !text.isEmpty { store.today.notes = text }
            }
            isRecording = true
            speechError = nil
        } catch ObservationDictationError.optedOut {
            speechError = Localizer.tr("quicklog_dictation_off")
        } catch {
            speechError = "Microphone or speech permission is required in Settings."
        }
    }
}

@MainActor
final class ObservationDictationController: ObservableObject {
    private var recognizer = SFSpeechRecognizer()
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var task: SFSpeechRecognitionTask?
    private let engine = AVAudioEngine()
    /// The Dictation opt-in (S-15). Injected so tests do not touch `UserDefaults.standard`.
    private let optIn: @MainActor () -> Bool

    init(optIn: @escaping @MainActor () -> Bool = { FeatureOptIns.shared.dictation }) {
        self.optIn = optIn
    }

    /// Throws `.optedOut` before asking iOS for anything while the opt-in is off (S-15).
    func start(onUpdate: @escaping (String) -> Void) async throws {
        guard optIn() else { throw ObservationDictationError.optedOut }
        let speech = await withCheckedContinuation { continuation in
            SFSpeechRecognizer.requestAuthorization { status in
                continuation.resume(returning: status)
            }
        }
        guard speech == .authorized else { throw ObservationDictationError.denied }

        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.record, mode: .measurement, options: .duckOthers)
        try session.setActive(true, options: .notifyOthersOnDeactivation)

        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        self.request = request
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in
            request.append(buffer)
        }
        engine.prepare()
        try engine.start()
        task = recognizer?.recognitionTask(with: request) { result, _ in
            if let result {
                onUpdate(result.bestTranscription.formattedString)
            }
        }
    }

    func stop() {
        engine.stop()
        engine.inputNode.removeTap(onBus: 0)
        request?.endAudio()
        task?.cancel()
        request = nil
        task = nil
        try? AVAudioSession.sharedInstance().setActive(false)
    }
}

enum ObservationDictationError: Error { case denied, optedOut }
