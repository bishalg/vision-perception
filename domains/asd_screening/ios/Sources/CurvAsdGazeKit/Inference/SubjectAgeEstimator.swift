import Foundation
import CoreVideo
import CoreGraphics

// MARK: - Subject Age Signal

/// Windowed, quality-gated subject age band signal produced by aggregating several
/// seconds of per-frame estimates. This is an assistive signal for setup guidance
/// and session metadata — never an age verification.
public struct SubjectAgeSignal: Codable, Sendable, Equatable {
    /// Modal band across the aggregation window (categorical median).
    public let band: FacialAgeBand
    /// Fraction of window samples agreeing with the modal band.
    public let agreement: Double
    /// Mean per-frame model confidence of the modal-band samples.
    public let confidence: Double
    /// Number of quality-gated estimates in the window.
    public let sampleCount: Int
    /// True when enough samples agree strongly enough to act on (soft prompts only).
    public let isStable: Bool
    public let timestamp: Date

    public init(
        band: FacialAgeBand,
        agreement: Double,
        confidence: Double,
        sampleCount: Int,
        isStable: Bool,
        timestamp: Date = Date()
    ) {
        self.band = band
        self.agreement = max(0.0, min(1.0, agreement))
        self.confidence = max(0.0, min(1.0, confidence))
        self.sampleCount = sampleCount
        self.isStable = isStable
        self.timestamp = timestamp
    }
}

// MARK: - Subject Age Estimator

/// Aggregates ≤5 Hz age-band inferences over a rolling ~2.5 s window into a
/// `SubjectAgeSignal`. Runs synchronously on the ARKit delegate queue: quality
/// gating first, then a single CoreImage crop + CoreML inference when the throttle
/// allows. When no model is installed, `record` is a near-zero-cost no-op.
///
/// Quality gates reuse the pediatric capture criteria (`GazeQualityGate` thresholds)
/// so age sampling only happens on frames where the gaze pipeline would also trust
/// the subject's pose.
public final class SubjectAgeEstimator: @unchecked Sendable {
    public struct Config: Sendable {
        /// Rolling window length in seconds.
        public var windowSec: Double = 2.5
        /// Minimum window samples before `isStable` can be true.
        public var minSamples: Int = 10
        /// Required fraction of window samples agreeing with the modal band.
        public var stabilityAgreement: Double = 0.70
        /// Minimum spacing between model inferences (≤5 Hz).
        public var inferenceIntervalSec: Double = 0.2
        public var minDistanceCm: Double = 30.0
        public var maxDistanceCm: Double = 75.0
        public var maxYawRad: Double = 0.61    // ~35°, matches GazeQualityGate
        public var maxPitchRad: Double = 0.52  // ~30°, matches GazeQualityGate
        public var minEyeOpenness: Double = 0.45

        public init() {}
    }

    private struct WindowSample: Sendable {
        let timestamp: TimeInterval
        let band: FacialAgeBand
        let confidence: Double
    }

    private let engine: any AgeBandInferring
    private let extractor: FaceCropExtractor
    private let rotation: FaceCropRotation
    private let config: Config

    private let lock = NSLock()
    private var window: [WindowSample] = []
    private var lastInferenceTimestamp: TimeInterval = -.infinity
    private var cachedSignal: SubjectAgeSignal?

    public init(
        engine: any AgeBandInferring,
        extractor: FaceCropExtractor = FaceCropExtractor(),
        rotation: FaceCropRotation = .clockwise90,
        config: Config = Config()
    ) {
        self.engine = engine
        self.extractor = extractor
        self.rotation = rotation
        self.config = config
    }

    /// True when an age model is loaded and the pipeline can produce signals.
    /// UI uses this to hide the subject-age affordances when no model is installed.
    public var isActive: Bool {
        engine.isModelAvailable
    }

    /// Latest windowed signal (nil before any accepted sample exists or when the
    /// window has drained, e.g. after the subject leaves the frame).
    public var currentSignal: SubjectAgeSignal? {
        lock.lock()
        defer { lock.unlock() }
        return cachedSignal
    }

    /// Clears the aggregation window (call between sessions/checks).
    public func reset() {
        lock.lock()
        defer { lock.unlock() }
        window.removeAll(keepingCapacity: false)
        lastInferenceTimestamp = -.infinity
        cachedSignal = nil
    }

    /// Records one ARKit frame. `capturedImage` is consumed synchronously on the
    /// caller's queue — ARKit reuses its backing store, so it must never be retained.
    ///
    /// Returns the current windowed signal, or nil when no accepted samples exist.
    @discardableResult
    public func record(
        capturedImage: CVPixelBuffer?,
        faceCenterNorm: CGPoint?,
        faceRadiusNorm: Double?,
        yawRad: Double,
        pitchRad: Double,
        distanceCm: Double,
        eyeOpenness: Double,
        timestamp: TimeInterval
    ) -> SubjectAgeSignal? {
        lock.lock()
        defer { lock.unlock() }

        prune(through: timestamp)

        // No model installed → skip all crop/render work; the pipeline is inert.
        guard engine.isModelAvailable else { return cachedSignal }

        guard let image = capturedImage else { return cachedSignal }
        let poseAccepted = abs(yawRad) <= config.maxYawRad && abs(pitchRad) <= config.maxPitchRad
        let distanceAccepted = distanceCm >= config.minDistanceCm && distanceCm <= config.maxDistanceCm
        let eyesAccepted = eyeOpenness >= config.minEyeOpenness
        guard poseAccepted, distanceAccepted, eyesAccepted else {
            return cachedSignal
        }

        guard timestamp - lastInferenceTimestamp >= config.inferenceIntervalSec - 1e-6 else {
            return cachedSignal
        }

        let faceRect: FaceCropExtractor.FaceRect
        if let center = faceCenterNorm {
            faceRect = FaceCropExtractor.FaceRect(
                centerXNorm: Double(center.x),
                centerYNorm: Double(center.y),
                radiusNorm: faceRadiusNorm ?? FaceCropExtractor.FaceRect.centered.radiusNorm
            )
        } else {
            faceRect = .centered
        }

        guard let crop = extractor.extractCrop(from: image, face: faceRect, rotation: rotation),
              let estimation = engine.estimate(crop: crop) else {
            return cachedSignal
        }

        lastInferenceTimestamp = timestamp
        window.append(WindowSample(timestamp: timestamp, band: estimation.band, confidence: estimation.confidence))
        cachedSignal = computeSignal()
        return cachedSignal
    }

    // MARK: - Window Math

    private func prune(through timestamp: TimeInterval) {
        let cutoff = timestamp - config.windowSec
        window.removeAll { $0.timestamp < cutoff }
        if window.isEmpty {
            cachedSignal = nil
        }
    }

    /// Modal band across the window; ties break toward the *younger* band so an
    /// ambiguous adult/toddler split never triggers an "adult holding the device"
    /// prompt. `agreement` is the modal share; `confidence` is the mean confidence
    /// of modal-band samples.
    private func computeSignal() -> SubjectAgeSignal? {
        guard !window.isEmpty else { return nil }

        var counts: [FacialAgeBand: Int] = [:]
        var confidences: [FacialAgeBand: [Double]] = [:]
        for sample in window {
            counts[sample.band, default: 0] += 1
            confidences[sample.band, default: []].append(sample.confidence)
        }

        let modal = FacialAgeBand.allCases
            .compactMap { band -> (band: FacialAgeBand, count: Int)? in
                guard let count = counts[band] else { return nil }
                return (band, count)
            }
            .max { lhs, rhs in
                if lhs.count == rhs.count { return lhs.band.ordinal > rhs.band.ordinal }
                return lhs.count < rhs.count
            }

        guard let modal else { return nil }

        let bandConfidences = confidences[modal.band] ?? []
        let meanConfidence = bandConfidences.isEmpty
            ? 0.0
            : bandConfidences.reduce(0.0, +) / Double(bandConfidences.count)
        let agreement = Double(modal.count) / Double(window.count)

        return SubjectAgeSignal(
            band: modal.band,
            agreement: agreement,
            confidence: meanConfidence,
            sampleCount: window.count,
            isStable: window.count >= config.minSamples && agreement >= config.stabilityAgreement
        )
    }
}
