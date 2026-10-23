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
/// which the AOI was active on a valid frame; clips where the AOI was never fixated report
/// the full observed duration as the latency (worst case for lower-is-better scoring).
public enum PerVideoMetricsExtractor {

    /// Fallback clip identifier for frames captured without a playlist (e.g. MainGazeScannerView).
    public static let untaggedClipId = "session_untagged"

    /// I-VT velocity threshold in NORMALIZED viewport units per second. The core C++
    /// OculomotorClassifier works in pixels (150 px/s on its AOI canvas); in [0,1] viewport
    /// coordinates a quarter-screen single-frame jump is ~15 u/s, so 5 u/s separates saccades
    /// from post-filter fixation jitter without missing real gaze shifts.
    static let velocityThresholdUnitsPerSec: Double = 5.0
    /// Minimum fixation duration in seconds (mirrors core OculomotorClassifier 80 ms).
    static let minFixationDurationSec: Double = 0.08
    static let frameIntervalSec: Double = 1.0 / 60.0

    public static func extract(from frames: [GazeFrame]) -> [PerVideoMetricSet] {
        var groups: [(id: String, frames: [GazeFrame])] = []
        var indexById: [String: Int] = [:]
        for frame in frames {
            let id = frame.stimulusId ?? untaggedClipId
            if let idx = indexById[id] {
                groups[idx].frames.append(frame)
            } else {
                indexById[id] = groups.count
                groups.append((id, [frame]))
            }
        }
        return groups.map { extractClip(stimulusId: $0.id, frames: $0.frames) }
    }

    // MARK: Per-clip computation

    static func extractClip(
        stimulusId: String,
        frames: [GazeFrame],
        timeOriginSec: Double = 0
    ) -> PerVideoMetricSet {
        let validFrames = frames.filter { $0.gaze.isValid && $0.qualityVerdict != .rejected }
        let validSampleRatio = frames.isEmpty ? 0.0 : Double(validFrames.count) / Double(frames.count)

        var metrics: [String: Double] = [:]
        var aoiDwellRatios: [String: Double] = [:]

        // -- AOI dwell ratios (per AOI, over valid frames) + first-entry latencies
        var aoiHitCounts: [String: Int] = [:]
        var aoiFirstEntry: [String: Double] = [:]
        for frame in validFrames {
            let t = mediaTime(of: frame)
            for name in frame.activeAOINames {
                aoiHitCounts[name, default: 0] += 1
                if let existing = aoiFirstEntry[name] {
                    aoiFirstEntry[name] = min(existing, t)
                } else {
                    aoiFirstEntry[name] = t
                }
            }
        }
        let denominator = max(1, validFrames.count)
        for (name, hits) in aoiHitCounts {
            let ratio = Double(hits) / Double(denominator)
            aoiDwellRatios[name] = ratio
            metrics[PerVideoMetricKey.aoiDwellRatio.key(targetAOI: name)] = ratio
        }
        let observedDuration = observedDurationSec(of: frames)
        for (name, entry) in aoiFirstEntry {
            let latencySec = max(0, entry - timeOriginSec)
            metrics[PerVideoMetricKey.firstEntryLatencyMs.key(targetAOI: name)] = latencySec * 1000.0
        }
        // AOIs that were never fixated emit no latency key; the scoring engine treats a
        // missing first-entry latency as the full clip duration (worst case, lower-is-better).

        // -- GeoPref-style social preference (meaningful for split-screen clips, harmless elsewhere)
        let socialHits = Double(aoiHitCounts["Social_Actor"] ?? 0)
        let geometricHits = Double(aoiHitCounts["Geometric_Pattern"] ?? 0)
        let totalGeoDwell = socialHits + geometricHits
        metrics[PerVideoMetricKey.socialDwellRatio.key(targetAOI: nil)] = denominator > 0 ? socialHits / Double(denominator) : 0
        metrics[PerVideoMetricKey.geometricDwellRatio.key(targetAOI: nil)] = denominator > 0 ? geometricHits / Double(denominator) : 0
        metrics[PerVideoMetricKey.socialPreferenceScore.key(targetAOI: nil)] =
            totalGeoDwell > 0 ? (socialHits - geometricHits) / totalGeoDwell : 0

        // -- Face scanning ratios (eye vs mouth share of face hits, mirroring session aggregate)
        let eyeHits = aoiHitCounts.reduce(0.0) { partial, entry in
            partial + (AOIRole.matches(entry.key, kind: .eye) ? Double(entry.value) : 0)
        }
        let mouthHits = aoiHitCounts.reduce(0.0) { partial, entry in
            partial + (AOIRole.matches(entry.key, kind: .mouth) ? Double(entry.value) : 0)
        }
        let faceHits = eyeHits + mouthHits
        metrics[PerVideoMetricKey.eyeDwellRatio.key(targetAOI: nil)] = faceHits > 0 ? eyeHits / faceHits : 0
        metrics[PerVideoMetricKey.mouthDwellRatio.key(targetAOI: nil)] = faceHits > 0 ? mouthHits / faceHits : 0

        // -- Fixation stats (lightweight I-VT over monotonic time runs)
        let fixation = fixationStats(of: validFrames)
        metrics[PerVideoMetricKey.fixationCount.key(targetAOI: nil)] = Double(fixation.count)
        metrics[PerVideoMetricKey.meanFixationDurationMs.key(targetAOI: nil)] = fixation.meanDurationMs

        // -- Facial affect
        if !validFrames.isEmpty {
            let smiles = validFrames.map { $0.affect.socialSmileIntensity }
            let expressivity = validFrames.map { $0.affect.expressivityIndex }
            let meanSmile = smiles.reduce(0, +) / Double(smiles.count)
            let meanExpressivity = expressivity.reduce(0, +) / Double(expressivity.count)

            metrics[PerVideoMetricKey.socialSmileIntensityMean.key(targetAOI: nil)] = meanSmile
            // Clip-level smile reactivity: peak smile minus the initial 500 ms baseline
            // (DATA_STORAGE_AND_PIPELINE D3.3 formula, clip onset as the reference event).
            let baselineWindow: [Double] = validFrames.compactMap { frame in
                let local = mediaTime(of: frame) - timeOriginSec
                return local >= 0 && local <= 0.5 ? frame.affect.socialSmileIntensity : nil
            }
            let baselineMean = baselineWindow.isEmpty ? meanSmile : baselineWindow.reduce(0, +) / Double(baselineWindow.count)
            metrics[PerVideoMetricKey.smileReactivityIndex.key(targetAOI: nil)] =
                max(0.0, min(1.0, (smiles.max() ?? 0) - baselineMean))
            metrics[PerVideoMetricKey.flatAffectScore.key(targetAOI: nil)] =
                max(0.0, min(1.0, 1.0 - (meanExpressivity * 50.0)))
            metrics[PerVideoMetricKey.surpriseScoreMean.key(targetAOI: nil)] =
                validFrames.map { $0.affect.surpriseScore }.reduce(0, +) / Double(validFrames.count)
        } else {
            metrics[PerVideoMetricKey.socialSmileIntensityMean.key(targetAOI: nil)] = 0
            metrics[PerVideoMetricKey.smileReactivityIndex.key(targetAOI: nil)] = 0
            metrics[PerVideoMetricKey.flatAffectScore.key(targetAOI: nil)] = 1.0
            metrics[PerVideoMetricKey.surpriseScoreMean.key(targetAOI: nil)] = 0
        }

        metrics[PerVideoMetricKey.validSampleRatio.key(targetAOI: nil)] = validSampleRatio

        return PerVideoMetricSet(
            stimulusId: stimulusId,
            clipFrameCount: frames.count,
            validFrameCount: validFrames.count,
            validSampleRatio: validSampleRatio,
            observedDurationSec: observedDuration,
            aoiDwellRatios: aoiDwellRatios,
            metrics: metrics
        )
    }

    // MARK: Helpers

    /// Media clock time for latency/AOI alignment; session elapsed time as fallback.
    static func mediaTime(of frame: GazeFrame) -> Double {
        frame.gaze.clipMediaTimeSec ?? frame.gaze.videoTimeSec
    }

    static func observedDurationSec(of frames: [GazeFrame]) -> Double {
        if let maxMedia = frames.compactMap({ $0.gaze.clipMediaTimeSec }).max(), maxMedia > 0 {
            return maxMedia
        }
        return Double(frames.count) * frameIntervalSec
    }

    /// Contiguous fixation runs (velocity < threshold, duration >= minimum) over monotonic time segments.
    /// Media clocks wrap on loop, so segmentation restarts at any backwards time step.
    static func fixationStats(of validFrames: [GazeFrame]) -> (count: Int, meanDurationMs: Double) {
        guard validFrames.count > 1 else { return (validFrames.isEmpty ? 0 : 1, validFrames.isEmpty ? 0 : minFixationDurationSec * 1000) }

        var fixationDurations: [Double] = []
        var runStart: Int? = nil

        func closeRun(_ endExclusive: Int) {
            guard let start = runStart, endExclusive > start + 1 else {
                runStart = nil
                return
            }
            let durationSec = mediaTime(of: validFrames[endExclusive - 1]) - mediaTime(of: validFrames[start])
            let fallbackDuration = Double(endExclusive - start) * frameIntervalSec
            let duration = durationSec > 0 ? durationSec : fallbackDuration
            if duration >= minFixationDurationSec {
                fixationDurations.append(duration)
            }
            runStart = nil
        }

        var previous: (point: (x: Double, y: Double), time: Double)? = nil
        for (idx, frame) in validFrames.enumerated() {
            let t = mediaTime(of: frame)
            let p = (x: frame.gaze.xNorm, y: frame.gaze.yNorm)

            if let prev = previous {
                let dt = t - prev.time
                if dt <= 0 {
                    // Loop wrap or clock glitch: close the run, restart segmentation
                    closeRun(idx)
                    previous = (p, t)
                    continue
                }
                let distance = ((p.x - prev.point.x) * (p.x - prev.point.x) + (p.y - prev.point.y) * (p.y - prev.point.y)).squareRoot()
                let velocity = distance / dt
                if velocity >= velocityThresholdUnitsPerSec {
                    // Saccade: close any open fixation run
                    closeRun(idx)
                    runStart = nil
                } else if runStart == nil {
                    runStart = idx - 1
                }
            } else {
                runStart = idx
            }
            previous = (p, t)
        }
        closeRun(validFrames.count)

        let meanMs = fixationDurations.isEmpty
            ? 0.0
            : fixationDurations.reduce(0, +) / Double(fixationDurations.count) * 1000.0
        return (fixationDurations.count, meanMs)
    }
}
