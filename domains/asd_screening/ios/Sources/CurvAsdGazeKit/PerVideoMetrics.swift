import Foundation

// MARK: - Per-Video Metric Model

/// Canonical metric keys shared between the extractor and normative baseline expectations.
///
/// AOI-specific metrics are emitted with a `::<AOI>` suffix (e.g. `first_entry_latency_ms::Gaze_Target_Right`);
/// baseline expectations pair a `biomarker` key with an optional `targetAOI` to compose the lookup key.
public enum PerVideoMetricKey: String, CaseIterable, Sendable {
    case socialDwellRatio = "social_dwell_ratio"
    case geometricDwellRatio = "geometric_dwell_ratio"
    case socialPreferenceScore = "social_preference_score"
    case eyeDwellRatio = "eye_dwell_ratio"
    case mouthDwellRatio = "mouth_dwell_ratio"
    case aoiDwellRatio = "aoi_dwell_ratio"
    case aoiDwellWindowRatio = "aoi_dwell_window_ratio"
    case firstEntryLatencyMs = "first_entry_latency_ms"
    case fixationCount = "fixation_count"
    case meanFixationDurationMs = "mean_fixation_duration_ms"
    case socialSmileIntensityMean = "social_smile_intensity_mean"
    case smileReactivityIndex = "smile_reactivity_index"
    case flatAffectScore = "flat_affect_score"
    case surpriseScoreMean = "surprise_score_mean"
    case validSampleRatio = "valid_sample_ratio"

    /// Composes the concrete metrics-dictionary key for a baseline expectation.
    public func key(targetAOI: String?) -> String {
        guard let aoi = targetAOI, !aoi.isEmpty else { return rawValue }
        return "\(rawValue)::\(aoi)"
    }
}

/// Metrics computed for a single stimulus clip from stimulus-tagged gaze frames.
/// Codable so per-clip results can be persisted in the vault and exported to the research server.
public struct PerVideoMetricSet: Codable, Sendable, Equatable {
    /// Playlist video name the frames were tagged with (or `PerVideoMetricsExtractor.untaggedClipId`).
    public let stimulusId: String
    public let clipFrameCount: Int
    public let validFrameCount: Int
    public let validSampleRatio: Double
    /// Longest observed clip time (media clock when available, else frame-count derived).
    public let observedDurationSec: Double
    /// Convenience mirror of the `aoi_dwell_ratio::<AOI>` entries (AOI name → ratio over valid frames).
    public let aoiDwellRatios: [String: Double]
    /// All extracted metrics keyed by `PerVideoMetricKey.key(targetAOI:)` composition.
    public let metrics: [String: Double]

    public init(
        stimulusId: String,
        clipFrameCount: Int,
        validFrameCount: Int,
        validSampleRatio: Double,
        observedDurationSec: Double,
        aoiDwellRatios: [String: Double],
        metrics: [String: Double]
    ) {
        self.stimulusId = stimulusId
        self.clipFrameCount = clipFrameCount
        self.validFrameCount = validFrameCount
        self.validSampleRatio = validSampleRatio
        self.observedDurationSec = observedDurationSec
        self.aoiDwellRatios = aoiDwellRatios
        self.metrics = metrics
    }
}

// MARK: - Extractor

/// Groups stimulus-tagged `GazeFrame`s by clip and computes per-clip clinical metrics.
///
/// Time source: `gaze.clipMediaTimeSec` (AVPlayer media clock) when present, else
/// `gaze.videoTimeSec` (session elapsed). Latency to an AOI is the earliest media time at
/// which the AOI was active on a valid frame; AOIs never fixated emit no latency key, and
/// scoring imputes the full observed duration (worst case for lower-is-better scoring).
///
/// The extraction (grouping, I-VT fixations, dwell, latency, affect) lives in the Rust core
/// (`asd-core::metrics`, CORE-5); this type keeps the public Swift API (FFI-3).
public enum PerVideoMetricsExtractor {

    /// Fallback clip identifier for frames captured without a playlist (e.g. MainGazeScannerView).
    public static let untaggedClipId = "session_untagged"

    public static func extract(from frames: [GazeFrame]) -> [PerVideoMetricSet] {
        extract(coreFrames: frames.map { CoreFrame($0) })
    }

    static func extract(coreFrames: [CoreFrame]) -> [PerVideoMetricSet] {
        do {
            return try AsdCoreBridge.extractMetrics(coreFrames)
        } catch {
            AsdCoreBridge.report("extract_metrics", error)
            return []
        }
    }
}
