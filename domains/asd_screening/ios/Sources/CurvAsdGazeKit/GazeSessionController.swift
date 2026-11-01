import AsdCore
import Foundation
import CoreGraphics
import CoreVideo
import Combine
#if canImport(ARKit) && !os(macOS)
import ARKit
#endif

/// Summary report produced at the conclusion of a gaze observation session.
///
/// Every stored property is required by the initialiser (PLC-1, SIM-4): there are no
/// placeholder defaults. A metric that was not measured is `nil` and must be omitted
/// by every export (PLC-3). Tests build instances through a fixture helper that lives
/// in the test target.
///
/// Scoring outcome (session schema 1.8, SCH-5): the five score values
/// (`socialPreferenceScore`, `socialSmileIntensityMean`, `smileReactivityIndex`,
/// `flatAffectScore`, `surpriseScoreMean`) are present together on a `scored` summary and
/// all `nil` on an `unscorable` one, which carries the `unscorableReason` instead. Nothing
/// substitutes a number for a score that was not computed (CON-14).
public struct GazeSessionReportSummary: Codable, Sendable, Equatable {
    public let sessionId: String
    /// Capture provenance (SIM-4). Simulated summaries are refused by every clinical sink.
    public let captureSource: CaptureSource
    public let durationSec: Double
    public let totalFrames: Int
    public let validFrames: Int
    public let validRatio: Double
    /// `.unscorable` when on-device scoring did not produce the five score values.
    public let scoringStatus: ScoringStatus
    /// Why the session is unscorable; `nil` on a scored session.
    public let unscorableReason: UnscorableReason?
    /// `nil` when `unscorable`.
    public let socialPreferenceScore: Double?
    public let socialDwellSec: Double
    public let geometricDwellSec: Double
    /// Share of eye-region hits among eye+mouth hits; `nil` when no face AOI hit occurred (PLC-2).
    public let eyeDwellRatio: Double?
    /// Share of mouth-region hits among eye+mouth hits; `nil` when no face AOI hit occurred (PLC-2).
    public let mouthDwellRatio: Double?
    /// First-look latency to the eye region. `nil` until a real first-fixation
    /// detector exists (PLC-1); never a constant.
    public let firstEyeLatencyMs: Double?
    /// `nil` when `unscorable`.
    public let socialSmileIntensityMean: Double?
    /// `nil` when `unscorable`.
    public let smileReactivityIndex: Double?
    /// `nil` when `unscorable`.
    public let flatAffectScore: Double?
    /// `nil` when `unscorable`.
    public let surpriseScoreMean: Double?
    public let qualityVerdict: GazeQualityGate.Verdict
    /// Assistive on-device subject age-band signal (nil when no age model is
    /// installed, estimation is disabled, or the window never stabilized). Band +
    /// confidence floats only — no face imagery ever persists.
    public let subjectAgeSignal: SubjectAgeSignal?
    /// Number of camera-session interruptions during recording (CAM-2; filled by WP-2.1).
    public let interruptionCount: Int
    /// Seconds spent interrupted during recording (CAM-2; filled by WP-2.1).
    public let interruptedSec: Double
    /// The session `trackingQuality` record (QS-1) from the Rust gaze pipeline, carried
    /// verbatim; `nil` when the pipeline did not run (simulated, or a summary from before
    /// WP-G.9).
    public let trackingQuality: JSONValue?
    /// Per-clip tracking values (QS-3, CNF-3) in play order; `nil` when the pipeline did not run.
    public let clipTracking: [ClipTrackingEntry]?

    public init(
        sessionId: String,
        captureSource: CaptureSource,
        durationSec: Double,
        totalFrames: Int,
        validFrames: Int,
        validRatio: Double,
        socialPreferenceScore: Double?,
        socialDwellSec: Double,
        geometricDwellSec: Double,
        eyeDwellRatio: Double?,
        mouthDwellRatio: Double?,
        firstEyeLatencyMs: Double?,
        socialSmileIntensityMean: Double?,
        smileReactivityIndex: Double?,
        flatAffectScore: Double?,
        surpriseScoreMean: Double?,
        unscorableReason: UnscorableReason? = nil,
        qualityVerdict: GazeQualityGate.Verdict,
        subjectAgeSignal: SubjectAgeSignal?,
        interruptionCount: Int,
        interruptedSec: Double,
        trackingQuality: JSONValue? = nil,
        clipTracking: [ClipTrackingEntry]? = nil
    ) {
        self.sessionId = sessionId
        self.captureSource = captureSource
        self.durationSec = durationSec
        self.totalFrames = totalFrames
        self.validFrames = validFrames
        self.validRatio = validRatio
        // Scored only with all five values and no reason; otherwise none of them (SCH-5).
        let scores = unscorableReason == nil ? SessionScores(
            socialPreferenceScore: socialPreferenceScore,
            socialSmileIntensityMean: socialSmileIntensityMean,
            smileReactivityIndex: smileReactivityIndex,
            flatAffectScore: flatAffectScore,
            surpriseScoreMean: surpriseScoreMean
        ) : nil
        self.scoringStatus = scores == nil ? .unscorable : .scored
        self.unscorableReason = scores == nil ? unscorableReason : nil
        self.socialPreferenceScore = scores?.socialPreferenceScore
        self.socialSmileIntensityMean = scores?.socialSmileIntensityMean
        self.smileReactivityIndex = scores?.smileReactivityIndex
        self.flatAffectScore = scores?.flatAffectScore
        self.surpriseScoreMean = scores?.surpriseScoreMean
        self.socialDwellSec = socialDwellSec
        self.geometricDwellSec = geometricDwellSec
        self.eyeDwellRatio = eyeDwellRatio
        self.mouthDwellRatio = mouthDwellRatio
        self.firstEyeLatencyMs = firstEyeLatencyMs
        self.qualityVerdict = qualityVerdict
        self.subjectAgeSignal = subjectAgeSignal
        self.interruptionCount = interruptionCount
        self.interruptedSec = interruptedSec
        self.trackingQuality = trackingQuality
        self.clipTracking = clipTracking
    }

    /// The five score values together; `nil` when `unscorable` (SCH-5).
    public var scores: SessionScores? {
        SessionScores(
            socialPreferenceScore: socialPreferenceScore,
            socialSmileIntensityMean: socialSmileIntensityMean,
            smileReactivityIndex: smileReactivityIndex,
            flatAffectScore: flatAffectScore,
            surpriseScoreMean: surpriseScoreMean
        )
    }

    private enum CodingKeys: String, CodingKey {
        case sessionId, captureSource, durationSec, totalFrames, validFrames, validRatio
        case scoringStatus, unscorableReason
        case socialPreferenceScore, socialDwellSec, geometricDwellSec
        case eyeDwellRatio, mouthDwellRatio, firstEyeLatencyMs
        case socialSmileIntensityMean, smileReactivityIndex, flatAffectScore, surpriseScoreMean
        case qualityVerdict, subjectAgeSignal, interruptionCount, interruptedSec
        case trackingQuality, clipTracking
    }

    /// Tolerates summary lines written before provenance and optional metrics existed
    /// (vault files from earlier builds): optional metrics and interruption fields
    /// decode as absent/zero. A missing `captureSource` decodes as `.trueDepth`
    /// (SIM-4 legacy rule); every summary written from now on carries it explicitly.
    ///
    /// Decision (WP-2.1, HANDOFF item 3): the default is kept rather than throwing.
    /// Neither the kit nor the app decodes summaries from the vault in a live path
    /// (`SessionVaultManager` only returns decrypted text, history uses
    /// `GazeSessionRecord`'s own decoder), so there is no live-vault decode where a
    /// throw could be scoped; a type-wide throw would make every pre-provenance
    /// vault line unreadable. Pre-provenance summaries were written by builds whose
    /// only recording path was TrueDepth or the then-unlabelled simulation fallback;
    /// any future clinical import of vault lines must re-check provenance from the
    /// header and the record's `legacyProvenance`, not from this default.
    /// Scoring outcome (1.8): a summary written before `scoringStatus` existed carried the
    /// five numbers and decodes as scored; an `unscorable` one decodes without them.
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let status = try c.decodeIfPresent(ScoringStatus.self, forKey: .scoringStatus) ?? .scored
        let reason = try c.decodeIfPresent(UnscorableReason.self, forKey: .unscorableReason)
        self.init(
            sessionId: try c.decode(String.self, forKey: .sessionId),
            captureSource: try c.decodeIfPresent(CaptureSource.self, forKey: .captureSource) ?? .trueDepth,
            durationSec: try c.decode(Double.self, forKey: .durationSec),
            totalFrames: try c.decode(Int.self, forKey: .totalFrames),
            validFrames: try c.decode(Int.self, forKey: .validFrames),
            validRatio: try c.decode(Double.self, forKey: .validRatio),
            socialPreferenceScore: try c.decodeIfPresent(Double.self, forKey: .socialPreferenceScore),
            socialDwellSec: try c.decode(Double.self, forKey: .socialDwellSec),
            geometricDwellSec: try c.decode(Double.self, forKey: .geometricDwellSec),
            eyeDwellRatio: try c.decodeIfPresent(Double.self, forKey: .eyeDwellRatio),
            mouthDwellRatio: try c.decodeIfPresent(Double.self, forKey: .mouthDwellRatio),
            firstEyeLatencyMs: try c.decodeIfPresent(Double.self, forKey: .firstEyeLatencyMs),
            socialSmileIntensityMean: try c.decodeIfPresent(Double.self, forKey: .socialSmileIntensityMean),
            smileReactivityIndex: try c.decodeIfPresent(Double.self, forKey: .smileReactivityIndex),
            flatAffectScore: try c.decodeIfPresent(Double.self, forKey: .flatAffectScore),
            surpriseScoreMean: try c.decodeIfPresent(Double.self, forKey: .surpriseScoreMean),
            unscorableReason: status == .unscorable ? (reason ?? .other) : nil,
            qualityVerdict: try c.decode(GazeQualityGate.Verdict.self, forKey: .qualityVerdict),
            subjectAgeSignal: try c.decodeIfPresent(SubjectAgeSignal.self, forKey: .subjectAgeSignal),
            interruptionCount: try c.decodeIfPresent(Int.self, forKey: .interruptionCount) ?? 0,
            interruptedSec: try c.decodeIfPresent(Double.self, forKey: .interruptedSec) ?? 0,
            trackingQuality: try c.decodeIfPresent(JSONValue.self, forKey: .trackingQuality),
            clipTracking: try c.decodeIfPresent([ClipTrackingEntry].self, forKey: .clipTracking)
        )
    }

}

/// Errors from the controller's persistence helpers.
public enum GazeSessionError: Error, Equatable {
    /// `stopAndPersistBatchAsync` was called although no session was started:
    /// there is nothing to write, and nothing may be labelled as captured.
    case nothingCaptured
}

/// Coordinates live 60 Hz gaze, facial affect, and head pose telemetry acquisition.
/// Live capture uses TrueDepth hardware (`ARSession`) and never falls back to
/// simulation; deterministic simulation exists only in Debug builds
/// (`ASD_SIMULATION_ENABLED`) behind `startSimulatedSession`.
///
/// Concurrency (CONC-1..4):
/// - The controller is `@MainActor`: it owns only UI-facing state (`@Published`).
/// - All per-frame state lives in `GazeProcessingActor`. Inputs reach it in order
///   through `ARSessionBridge` (the `ARSessionDelegate`), whose submit calls are
///   synchronous, so calibration/clip/clock updates and frames keep their order.
/// - UI comes back as coalesced `GazeUIState` snapshots applied on the main actor.
/// - `stopSession()` is `async`: it fixes the stop point synchronously, then awaits
///   the actor's `finalizeSession`, the only place a summary is aggregated.
@MainActor
public final class GazeSessionController: ObservableObject {
    @Published public var currentGazePoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    @Published public var currentRawGazePoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    @Published public private(set) var calibrationProfile: GazeCalibrationProfile
    @Published public var currentDistanceCm: Double = 48.0
    @Published public var currentQuality: GazeQualityGate.Evaluation
    @Published public var currentAffect: FacialAffectMetrics = .neutral
    @Published public var activeAOIs: [String] = []
    @Published public var isRecording: Bool = false
    @Published public var headYawDeg: Double = 0.0
    @Published public var headPitchDeg: Double = 0.0
    @Published public var headRollDeg: Double = 0.0

    /// Frames of the last finished session, published once at stop (never at 60 Hz).
    @Published public var recordedFrames: [GazeFrame] = []

    @Published public private(set) var trackingStage: TrackingStage = .searchingFace

    public let qualityGate: GazeQualityGate
    /// Main-actor copy of the active AOI battery (the actor holds the one used for hit testing).
    public private(set) var aoiTracker: StimulusAOITracker

    /// Single owner of per-frame state (CONC-1).
    public let processingActor: GazeProcessingActor

    /// Ordered ingress and `ARSessionDelegate` (CONC-2).
    public let sessionBridge: ARSessionBridge

    /// Assistive subject-age pipeline running off the ARFrame stream. Inert until an
    /// age model is installed (`AgeBandEstimationEngine.discoverCompiledModelURL()`)
    /// and, independently, until `isSubjectAgeEstimationEnabled` is set (CNS-8).
    public let subjectAgeEstimator: SubjectAgeEstimator

    /// Off by default (CNS-8): the estimator receives no frames unless a caller that
    /// discloses the age-range check (the quick-check card) turns this on for its
    /// own session. Turning it off drops the estimator's window.
    public var isSubjectAgeEstimationEnabled: Bool = false {
        didSet { subjectAgeEstimator.isEnabled = isSubjectAgeEstimationEnabled }
    }
    /// Latest windowed subject age-band signal (5 Hz max update rate).
    @Published public private(set) var subjectAgeSignal: SubjectAgeSignal?

    /// Wall-clock start of the active clip (informational; the pipeline uses the frame clock).
    public private(set) var clipStartTime: Date?

    /// Provenance of the current (or last) recording; `nil` before any session
    /// started or when live tracking was unavailable (SIM-2).
    public private(set) var captureSource: CaptureSource?

    /// Read-only compatibility shim for one release; use `captureSource`.
    public var isSimulationMode: Bool { captureSource == .simulated }
    @Published public var calibrationTarget: CGPoint? = nil

    /// Stimulus clip currently playing (playlist video name). Nil when no playlist context.
    public private(set) var currentStimulusId: String?
    /// AVPlayer media clock seconds within the active clip (wraps on loop). Nil until reported.
    public private(set) var clipMediaTime: Double?

    nonisolated public static let didUpdateCalibrationNotification = Notification.Name("CurvAsdGazeKit.calibrationDidUpdate")
    nonisolated public static let didUpdateFaceBlendShapesNotification = Notification.Name("CurvAsdGazeKit.faceBlendShapesDidUpdate")
    nonisolated public static let didStartRecordingNotification = Notification.Name("CurvAsdGazeKit.recordingDidStart")
    nonisolated public static let didStopRecordingNotification = Notification.Name("CurvAsdGazeKit.recordingDidStop")
    nonisolated public static let blendShapesUserInfoKey = "blendShapes"
    nonisolated public static let affectUserInfoKey = "affect"
    nonisolated public static let isRecordingUserInfoKey = "isRecording"
    nonisolated public static let sessionIdUserInfoKey = "sessionId"

    private var lastBlendShapeBroadcast: TimeInterval = 0
    private var recordingSessionToken: String = UUID().uuidString
    /// Bumped by every start; a stop that finishes after a newer start does not
    /// overwrite the newer session's published state.
    private var sessionGeneration: UInt64 = 0
    private let mailbox = PipelineMailbox()
    private var calibrationObserver: NotificationObserverToken?

    #if canImport(ARKit) && !os(macOS)
    @Published public var arSession: ARSession? {
        didSet { sessionForTeardown = arSession }
    }
    /// Mirror of `arSession` for `deinit` (nonisolated). Only touched on the main
    /// actor and in `deinit`, when no other reference to the controller exists.
    nonisolated(unsafe) private var sessionForTeardown: ARSession?
    private let arDelegateQueue = DispatchQueue(label: "org.curv.asd.arSessionQueue", qos: .userInteractive)
    #endif

    public init(
        qualityGate: GazeQualityGate = ScoringConfiguration.active.qualityGate,
        aoiTracker: StimulusAOITracker = StimulusAOITracker.makeGeoPrefBattery(),
        calibrationProfile: GazeCalibrationProfile = .identity,
        subjectAgeEstimator: SubjectAgeEstimator? = nil
    ) {
        self.qualityGate = qualityGate
        self.aoiTracker = aoiTracker
        self.calibrationProfile = calibrationProfile
        let actor = GazeProcessingActor(
            qualityGate: qualityGate,
            aoiTracker: aoiTracker,
            calibrationProfile: calibrationProfile
        )
        self.processingActor = actor
        let estimator = subjectAgeEstimator ?? SubjectAgeEstimator(engine: AgeBandEstimationEngine.shared)
        estimator.isEnabled = false
        self.subjectAgeEstimator = estimator
        self.currentQuality = GazeQualityGate.Evaluation(isValid: false, verdict: .rejected, failureReasons: ["Not started"])

        let mailbox = self.mailbox
        self.sessionBridge = ARSessionBridge(
            processingActor: actor,
            ageEstimator: estimator,
            uiSink: { mailbox.post(ui: $0) },
            ageSink: { mailbox.post(age: $0) }
        )
        mailbox.setDrain { [weak self] in self?.drainMailbox() }

        calibrationObserver = NotificationObserverToken(
            NotificationCenter.default.addObserver(
                forName: Self.didUpdateCalibrationNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.reloadCalibrationProfile()
                }
            }
        )
    }

    deinit {
        #if canImport(ARKit) && !os(macOS)
        sessionForTeardown?.pause()
        sessionForTeardown?.delegate = nil
        #endif
    }

    // MARK: - Calibration

    /// Participant and session the calibration of this controller belongs to (CAL-1).
    /// Without a binding the controller never loads or persists a stored profile.
    public var calibrationBinding: CalibrationBinding?

    /// Applies a freshly fitted profile and persists it under `calibrationBinding`.
    /// With no binding the profile is applied in memory only.
    public func applyAndPersist(calibrationProfile profile: GazeCalibrationProfile) {
        calibrationProfile = profile
        if let calibrationBinding {
            BoundCalibrationStore.save(profile, binding: calibrationBinding)
        }
        sessionBridge.updateCalibration(profile)
        NotificationCenter.default.post(name: GazeSessionController.didUpdateCalibrationNotification, object: nil)
    }

    /// Resets calibration profile to identity transform.
    public func resetCalibration() {
        calibrationProfile = .identity
        BoundCalibrationStore.reset()
        sessionBridge.resetCalibration()
        NotificationCenter.default.post(name: GazeSessionController.didUpdateCalibrationNotification, object: nil)
    }

    /// Reloads the profile saved for this controller's binding; identity if none (CAL-1).
    public func reloadCalibrationProfile() {
        // Unbound controllers keep their own in-memory profile and never read the store.
        guard let calibrationBinding else { return }
        calibrationProfile = BoundCalibrationStore.load(for: calibrationBinding)
        sessionBridge.updateCalibration(calibrationProfile)
    }

    // MARK: - Session lifecycle

    /// Starts live TrueDepth capture. Volatile RAM buffer is reset.
    ///
    /// Never falls back to simulation (SIM-2): when face tracking is unsupported the
    /// controller moves to `.unavailable(.faceTrackingUnsupported)` and does not record.
    ///
    /// - Parameter beginsClip: open the first stimulus clip now (default). Pass `false` when
    ///   the session starts with calibration and validation (WP-G.10); `switchStimulusClip`
    ///   then opens the first clip in the same pipeline, so the session records its own
    ///   calibration (CAL-1, QS-1).
    public func startSession(battery: StimulusAOITracker? = nil, stimulusId: String? = nil, beginsClip: Bool = true) {
        #if canImport(ARKit) && !os(macOS)
        guard ARFaceTrackingConfiguration.isSupported else {
            markUnavailable(.faceTrackingUnsupported)
            return
        }
        tearDownARSession()
        beginRecording(battery: battery, stimulusId: stimulusId, source: .trueDepth, beginsClip: beginsClip)

        let session = ARSession()
        session.delegateQueue = arDelegateQueue
        session.delegate = sessionBridge
        let config = ARFaceTrackingConfiguration()
        config.isWorldTrackingEnabled = false
        session.run(config, options: [.resetTracking, .removeExistingAnchors])
        self.arSession = session
        #else
        markUnavailable(.faceTrackingUnsupported)
        #endif
    }

    #if ASD_SIMULATION_ENABLED
    /// Starts a deterministic simulated session (Debug builds only, SIM-3). Frames
    /// arrive through `tickSimulatedFrame`; every summary is `.simulated`.
    public func startSimulatedSession(battery: StimulusAOITracker? = nil, stimulusId: String? = nil, beginsClip: Bool = true) {
        #if canImport(ARKit) && !os(macOS)
        tearDownARSession()
        #endif
        beginRecording(battery: battery, stimulusId: stimulusId, source: .simulated, beginsClip: beginsClip)
    }
    #endif

    /// Leaves the controller idle in an explicit unavailable state (ADR-0001 decision 2).
    private func markUnavailable(_ reason: TrackingUnavailableReason) {
        isRecording = false
        captureSource = nil
        trackingStage = .unavailable(reason: reason)
        currentQuality = GazeQualityGate.Evaluation(isValid: false, verdict: .rejected, failureReasons: [trackingStage.label])
    }

    private func beginRecording(battery: StimulusAOITracker?, stimulusId: String?, source: CaptureSource, beginsClip: Bool) {
        if let battery = battery {
            self.aoiTracker = battery
        }
        sessionGeneration &+= 1
        if let calibrationBinding {
            calibrationProfile = BoundCalibrationStore.load(for: calibrationBinding)
        }
        captureSource = source
        recordedFrames.removeAll()
        clipStartTime = Date()
        currentStimulusId = stimulusId
        clipMediaTime = nil

        // Enqueued before the AR session runs, so the actor is recording before the
        // first frame arrives.
        sessionBridge.start(
            battery: battery,
            profile: calibrationProfile,
            captureSource: source,
            stimulusId: stimulusId,
            originTimestamp: ProcessInfo.processInfo.systemUptime,
            orientation: Self.interfaceOrientation(),
            beginsClip: beginsClip
        )

        isRecording = true
        recordingSessionToken = UUID().uuidString
        NotificationCenter.default.post(name: Self.didStartRecordingNotification, object: self)

        trackingStage = .searchingFace
        currentQuality = GazeQualityGate.Evaluation(isValid: false, verdict: .rejected, failureReasons: ["Waiting for camera…"])
    }

    /// Dynamically switches the active stimulus clip and AOI battery mid-session.
    public func switchStimulusClip(battery: StimulusAOITracker, stimulusId: String? = nil) {
        aoiTracker = battery
        clipStartTime = Date()
        currentStimulusId = stimulusId
        clipMediaTime = nil
        sessionBridge.switchStimulusClip(
            battery: battery,
            stimulusId: stimulusId,
            at: captureSource == .trueDepth ? ProcessInfo.processInfo.systemUptime : nil
        )
    }

    /// Receives the authoritative AVPlayer media clock for the active clip (10 Hz periodic observer).
    /// Time-coded AOI windows and per-clip segmentation align to this clock, not wall-clock.
    public func updateClipMediaTime(_ timeSec: Double) {
        clipMediaTime = timeSec
        sessionBridge.updateClipMediaTime(timeSec)
    }

    /// Ends the playing clip without starting another (QS-3): call it when the clip stops and
    /// before the drift check that follows it. `switchStimulusClip` opens the next clip.
    public func endStimulusClip() {
        sessionBridge.endStimulusClip()
    }

    /// The interface orientation changed during capture (GEO-3).
    public func updateInterfaceOrientation(_ orientation: GazeOrientation) {
        sessionBridge.updateOrientation(orientation)
    }

    /// The foreground scene's orientation on iOS; `nil` elsewhere (the pipeline keeps its own).
    private static func interfaceOrientation() -> GazeOrientation? {
        #if canImport(UIKit) && !os(watchOS) && !os(visionOS)
        return GazeDeviceInfo.currentInterfaceOrientation()
        #else
        return nil
        #endif
    }

    // MARK: - Targets (calibration, validation, drift checks; PIPE-1, for WP-G.10)
    //
    // Each call runs on the processing actor in order with the frames already submitted, so
    // a target window opens exactly at the next frame after the call.

    /// Starts a (re-)calibration in the session pipeline.
    public func startCalibration(layout: [GazeVec2]? = nil, reason: GazeRecalReason? = nil) async throws {
        let result: Result<Void, any Error>? = await sessionBridge.perform { actor in
            do { return .success(try await actor.startCalibration(layout: layout, reason: reason)) } catch { return .failure(error) }
        }
        try (result ?? .failure(GazeProcessingActor.TargetError.noPipeline)).get()
    }

    /// What the calibration UI should show next; `nil` when no calibration is running.
    public func calibrationStatus() async -> GazeCalibrationStatus? {
        await sessionBridge.perform { await $0.calibrationStatus() } ?? nil
    }

    /// Opens a calibration, validation or drift-check window at the next frame.
    public func beginTarget(kind: GazeTargetKind, at target: GazeVec2) async throws {
        let result: Result<Void, any Error>? = await sessionBridge.perform { actor in
            do { return .success(try await actor.beginTarget(kind: kind, at: target)) } catch { return .failure(error) }
        }
        try (result ?? .failure(GazeProcessingActor.TargetError.noPipeline)).get()
    }

    /// Whether the open window has collected its frame time (then call `endTarget`).
    public func targetComplete() async -> Bool {
        await sessionBridge.perform { await $0.targetComplete() } ?? false
    }

    /// Closes the open window and returns its result.
    public func endTarget() async -> GazeTargetResultRecord? {
        await sessionBridge.perform { await $0.endTarget() } ?? nil
    }

    /// Fits, scores and (unless it failed) applies the running calibration.
    public func finishCalibration() async throws -> GazeCalibrationOutcomeRecord {
        let result: Result<GazeCalibrationOutcomeRecord, any Error>? = await sessionBridge.perform { actor in
            do { return .success(try await actor.finishCalibration()) } catch { return .failure(error) }
        }
        return try (result ?? .failure(GazeProcessingActor.TargetError.noPipeline)).get()
    }

    /// Re-calibration advice (CAL-9, VAL-4, DRF-3); act on it only at a clip boundary.
    public func recalibrationAdvice() async -> GazeRecalAdviceRecord? {
        await sessionBridge.perform { await $0.recalibrationAdvice() } ?? nil
    }

    /// Where to show the next drift check target.
    public func nextCheckTarget() async -> GazeVec2? {
        await sessionBridge.perform { await $0.nextCheckTarget() } ?? nil
    }

    /// Validation of the calibration in effect (VAL-2, VAL-3); `nil` before any validation target.
    public func validationSummary() async -> GazeValidationSummary? {
        await sessionBridge.perform { await $0.validationSummary() } ?? nil
    }

    /// Target layouts and window timing of the session pipeline (CAL-7, VAL-1, DRF-1).
    public func targetSchedule() async -> GazeTargetSchedule? {
        await sessionBridge.perform { await $0.targetSchedule() } ?? nil
    }

    /// Ends the session and returns its summary (CONC-4).
    ///
    /// The stop point is fixed synchronously when this is called: frames the bridge
    /// received before are included, frames after are dropped. The summary is
    /// aggregated by the actor (`finalizeSession`). Without a started session the
    /// summary is empty, `.rejected` and labelled `.simulated` (quarantined): it
    /// never claims live data.
    public func stopSession() async -> GazeSessionReportSummary {
        let generation = sessionGeneration
        let ageSignal = isSubjectAgeEstimationEnabled ? subjectAgeEstimator.currentSignal : nil
        let pending = sessionBridge.requestFinalize(
            at: ProcessInfo.processInfo.systemUptime,
            subjectAgeSignal: ageSignal
        )

        isRecording = false
        NotificationCenter.default.post(name: Self.didStopRecordingNotification, object: self)
        #if canImport(ARKit) && !os(macOS)
        tearDownARSession()
        #endif

        let (summary, frames) = await pending.value
        if generation == sessionGeneration {
            recordedFrames = frames
        }
        return summary
    }

    /// Asynchronously stops session and persists the volatile RAM buffer as an encrypted batch to disk.
    ///
    /// Throws `GazeSessionError.nothingCaptured` when no session was started.
    public func stopAndPersistBatchAsync(
        participantToken: String,
        stimulusBattery: String = "GeoPref_v1",
        vault: SessionVaultManager = SessionVaultManager()
    ) async throws -> (summary: GazeSessionReportSummary, vaultURL: URL) {
        let started = captureSource != nil && isRecording
        let summary = await stopSession()
        guard started else { throw GazeSessionError.nothingCaptured }
        let url = try await vault.saveSessionBatchAsync(
            header: makeHeader(for: summary, participantToken: participantToken, stimulusBattery: stimulusBattery),
            frames: recordedFrames,
            report: summary
        )
        return (summary, url)
    }

    #if ASD_RESEARCH_CLASSIFIER
    /// Research builds only (CLM-3): also persists a classifier result line.
    public func stopAndPersistBatchAsync(
        participantToken: String,
        stimulusBattery: String = "GeoPref_v1",
        vault: SessionVaultManager = SessionVaultManager(),
        classification: BiomarkerClassificationResult?
    ) async throws -> (summary: GazeSessionReportSummary, vaultURL: URL) {
        let started = captureSource != nil && isRecording
        let summary = await stopSession()
        guard started else { throw GazeSessionError.nothingCaptured }
        let url = try await vault.saveSessionBatchAsync(
            header: makeHeader(for: summary, participantToken: participantToken, stimulusBattery: stimulusBattery),
            frames: recordedFrames,
            report: summary,
            classification: classification
        )
        return (summary, url)
    }
    #endif

    private func makeHeader(
        for summary: GazeSessionReportSummary,
        participantToken: String,
        stimulusBattery: String
    ) -> SessionHeader {
        SessionHeader(
            sessionId: summary.sessionId,
            participantToken: participantToken,
            stimulusBattery: stimulusBattery,
            stimulusDurationSec: summary.durationSec,
            captureSource: summary.captureSource
        )
    }

    #if canImport(ARKit) && !os(macOS)
    private func tearDownARSession() {
        guard let existing = arSession else { return }
        existing.pause()
        existing.delegate = nil
        arSession = nil
    }
    #endif

    // MARK: - Simulation (Debug)

    /// Ingests a single synthetic frame (Debug simulation, unit tests).
    ///
    /// No-op unless the controller is in a session started by `startSimulatedSession`
    /// (SIM-3). In builds without `ASD_SIMULATION_ENABLED` the body is compiled out,
    /// so the Release binary contains no frame synthesiser. The frame is processed by
    /// the actor in order with every other pipeline input; UI updates follow
    /// asynchronously (tests use `flushPipeline()`).
    public func tickSimulatedFrame(
        timeSec: Double,
        clipTimeSec: Double? = nil,
        stimulusId: String? = nil,
        clipMediaTimeSec: Double? = nil
    ) {
        #if ASD_SIMULATION_ENABLED
        guard captureSource == .simulated else { return }
        sessionBridge.submitSimulated(
            timeSec: timeSec,
            target: calibrationTarget,
            clipTimeSec: clipTimeSec,
            stimulusId: stimulusId ?? currentStimulusId,
            clipMediaTimeSec: clipMediaTimeSec ?? clipMediaTime
        )
        #endif
    }

    /// Waits until every pipeline input submitted so far has been processed and its
    /// UI state applied. For tests and deterministic hand-offs; not needed in the app.
    func flushPipeline() async {
        await sessionBridge.drain()
        drainMailbox()
    }

    // MARK: - Subject age (CNS-8)

    /// Feeds one camera frame to the subject-age estimator, only when
    /// `isSubjectAgeEstimationEnabled` (CNS-8). The AR bridge feeds the estimator
    /// directly on its queue (gated by the estimator's own `isEnabled`); this entry
    /// point keeps the gate testable on macOS.
    @discardableResult
    func ingestSubjectAgeFrame(
        capturedImage: CVPixelBuffer?,
        faceCenterNorm: CGPoint?,
        faceRadiusNorm: Double?,
        yawRad: Double,
        pitchRad: Double,
        distanceCm: Double,
        eyeOpenness: Double,
        timestamp: TimeInterval
    ) -> SubjectAgeSignal? {
        guard isSubjectAgeEstimationEnabled else { return nil }
        return subjectAgeEstimator.record(
            capturedImage: capturedImage,
            faceCenterNorm: faceCenterNorm,
            faceRadiusNorm: faceRadiusNorm,
            yawRad: yawRad,
            pitchRad: pitchRad,
            distanceCm: distanceCm,
            eyeOpenness: eyeOpenness,
            timestamp: timestamp
        )
    }

    // MARK: - UI hand-off

    /// Applies the newest pipeline output (coalesced by the mailbox).
    private func drainMailbox() {
        let (ui, age) = mailbox.take()
        if let ui {
            applyUIState(ui)
            broadcastBlendShapes(affect: ui.currentAffect, shapes: ui.blendShapes)
        }
        if let age, isSubjectAgeEstimationEnabled, subjectAgeSignal != age {
            subjectAgeSignal = age
        }
    }

    /// Consolidates UI property assignments on `@MainActor`.
    private func applyUIState(_ state: GazeUIState) {
        currentGazePoint = state.currentGazePoint
        currentRawGazePoint = state.currentRawGazePoint
        currentDistanceCm = state.distanceCm
        currentQuality = state.currentQuality
        currentAffect = state.currentAffect
        activeAOIs = state.activeAOIs
        trackingStage = state.trackingStage
        headYawDeg = state.headYawDeg
        headPitchDeg = state.headPitchDeg
        headRollDeg = state.headRollDeg
    }

    /// Throttled numeric blend-shape fan-out for mood fusion and Core ML CSV (no frames).
    func broadcastBlendShapes(affect: FacialAffectMetrics, shapes: [String: Double]) {
        let now = Date().timeIntervalSince1970
        guard now - lastBlendShapeBroadcast >= 0.2 else { return }
        lastBlendShapeBroadcast = now
        var info: [String: Any] = [
            Self.affectUserInfoKey: affect,
            Self.blendShapesUserInfoKey: shapes,
            Self.isRecordingUserInfoKey: isRecording,
            Self.sessionIdUserInfoKey: recordingSessionToken
        ]
        if shapes.isEmpty {
            info[Self.blendShapesUserInfoKey] = blendShapeMap(from: affect)
        }
        NotificationCenter.default.post(
            name: Self.didUpdateFaceBlendShapesNotification,
            object: self,
            userInfo: info
        )
    }

    private func blendShapeMap(from affect: FacialAffectMetrics) -> [String: Double] {
        [
            "mouthSmileLeft": affect.mouthSmileLeft,
            "mouthSmileRight": affect.mouthSmileRight,
            "cheekSquintLeft": affect.cheekSquintLeft,
            "cheekSquintRight": affect.cheekSquintRight,
            "browInnerUp": affect.browInnerUp,
            "browOuterUpLeft": affect.browOuterUpLeft,
            "browOuterUpRight": affect.browOuterUpRight,
            "eyeWideLeft": affect.eyeWideLeft,
            "eyeWideRight": affect.eyeWideRight,
            "jawOpen": affect.jawOpen,
            "browDownLeft": affect.browDownLeft,
            "browDownRight": affect.browDownRight,
            "mouthFrownLeft": affect.mouthFrownLeft,
            "mouthFrownRight": affect.mouthFrownRight
        ]
    }
}

/// Removes a block-based NotificationCenter observer when released.
/// `NotificationCenter.removeObserver` is thread-safe; the token is immutable.
private final class NotificationObserverToken: @unchecked Sendable {
    private let token: NSObjectProtocol

    init(_ token: NSObjectProtocol) {
        self.token = token
    }

    deinit {
        NotificationCenter.default.removeObserver(token)
    }
}

/// The app drives calibration, validation and drift checks through the controller (WP-G.10).
extension GazeSessionController: GazeTargetSession {}
