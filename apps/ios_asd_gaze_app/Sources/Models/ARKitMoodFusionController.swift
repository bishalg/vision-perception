import Foundation
import Combine
import CurvAsdGazeKit

/// Listens to TrueDepth blend-shape updates and suggests a diary log after
/// sustained browInnerUp + mouthFrown (default > 2 minutes while recording,
/// or ≥ 8 seconds accumulated at session end).
@MainActor
public final class ARKitMoodFusionController: ObservableObject {
    public static let shared = ARKitMoodFusionController()

    @Published public var pendingSuggestion: MoodDiarySuggestion?
    @Published public private(set) var consecutiveDistressSeconds: TimeInterval = 0
    @Published public private(set) var sessionDistressSeconds: TimeInterval = 0

    private var lastSampleAt: Date?
    private var lastNotifyAt: Date?
    private var observers: [NSObjectProtocol] = []
    private var firedSustainedThisSession = false

    public init() {}

    public func startListening() {
        guard observers.isEmpty else { return }
        observers.append(
            NotificationCenter.default.addObserver(
                forName: GazeSessionController.didUpdateFaceBlendShapesNotification,
                object: nil,
                queue: .main
            ) { [weak self] note in
                // `Notification` is not `Sendable`: read its Sendable payload here and send
                // only that to the main actor (Swift 6 region isolation, CONC-5).
                let payload = Self.blendShapePayload(from: note)
                Task { @MainActor in
                    self?.handleBlendShapePayload(payload)
                }
            }
        )
        observers.append(
            NotificationCenter.default.addObserver(
                forName: GazeSessionController.didStopRecordingNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor in
                    self?.handleSessionStop()
                }
            }
        )
        observers.append(
            NotificationCenter.default.addObserver(
                forName: GazeSessionController.didStartRecordingNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor in
                    self?.resetSession()
                }
            }
        )
    }

    public func stopListening() {
        observers.forEach { NotificationCenter.default.removeObserver($0) }
        observers.removeAll()
    }

    public func dismissSuggestion() {
        pendingSuggestion = nil
    }

    public func handleRecordingDidStop() {
        handleSessionStop()
    }

    /// Testable ingest path (no ARKit).
    public func ingestFrame(
        at date: Date,
        affect: FacialAffectMetrics,
        isRecording: Bool
    ) {
        guard isRecording else { return }
        let dt: TimeInterval
        if let last = lastSampleAt {
            dt = min(1.0, max(0, date.timeIntervalSince(last)))
        } else {
            dt = 1.0 / 5.0
        }
        lastSampleAt = date

        if DistressBlendShapeHeuristic.isDistress(from: affect) {
            consecutiveDistressSeconds += dt
            sessionDistressSeconds += dt
        } else {
            consecutiveDistressSeconds = 0
        }

        if !firedSustainedThisSession,
           consecutiveDistressSeconds >= DistressBlendShapeHeuristic.sustainedPromptSeconds {
            firedSustainedThisSession = true
            emitSuggestion(seconds: consecutiveDistressSeconds)
        }
    }

    /// The affect sample and recording flag of a blend-shape notification; `nil` when the
    /// notification carries no affect sample.
    private nonisolated static func blendShapePayload(
        from note: Notification
    ) -> (affect: FacialAffectMetrics, isRecording: Bool)? {
        guard let affect = note.userInfo?[GazeSessionController.affectUserInfoKey] as? FacialAffectMetrics else {
            return nil
        }
        let recording = (note.userInfo?[GazeSessionController.isRecordingUserInfoKey] as? Bool) ?? false
        return (affect, recording)
    }

    private func handleBlendShapePayload(_ payload: (affect: FacialAffectMetrics, isRecording: Bool)?) {
        guard let payload else { return }
        ingestFrame(at: Date(), affect: payload.affect, isRecording: payload.isRecording)
    }

    private func handleSessionStop() {
        if !firedSustainedThisSession,
           sessionDistressSeconds >= DistressBlendShapeHeuristic.sessionEndPromptSeconds {
            emitSuggestion(seconds: sessionDistressSeconds)
        }
        lastSampleAt = nil
        consecutiveDistressSeconds = 0
    }

    private func resetSession() {
        lastSampleAt = nil
        consecutiveDistressSeconds = 0
        sessionDistressSeconds = 0
        firedSustainedThisSession = false
    }

    private func emitSuggestion(seconds: TimeInterval) {
        if let lastNotifyAt, Date().timeIntervalSince(lastNotifyAt) < 60 { return }
        lastNotifyAt = Date()
        let intensity = min(1, 0.45 + seconds / 180.0)
        let emotion: PediatricEmotion = seconds >= DistressBlendShapeHeuristic.sustainedPromptSeconds
            ? .meltdown
            : .overstimulated
        pendingSuggestion = MoodDiarySuggestion(
            distressSeconds: seconds,
            suggestedEmotion: emotion,
            intensityHint: intensity
        )
    }
}
