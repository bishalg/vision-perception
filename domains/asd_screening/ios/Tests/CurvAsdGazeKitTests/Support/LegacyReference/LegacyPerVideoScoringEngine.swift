// Former Swift body of `PerVideoScoringEngine` at dev@3718ea6, moved here by WP-A.11 (FFI-3).
// The production type now delegates to the Rust core; this copy is the parity oracle for
// one release (GoldenParityTests, LegacyParityTests). Do not change its behaviour.
@testable import CurvAsdGazeKit
import Foundation

enum LegacyPerVideoScoringEngine {

    /// Clips below this valid-sample ratio are marked low-confidence (matches the 0.60 vault gate).
    static let lowConfidenceValidRatio: Double = 0.60

    static func score(
        metricSets: [PerVideoMetricSet],
        baseline: NormativeBaselineSet,
        ageMonths: Int,
        frames: [GazeFrame] = [],
        scoredAt: Date = Date()
    ) -> PerVideoSessionScore {
        let bracketId = resolvedBracketId(baseline: baseline, ageMonths: ageMonths)
        let framesByClip = Dictionary(grouping: frames) { frame in
            frame.stimulusId ?? LegacyPerVideoMetricsExtractor.untaggedClipId
        }

        var reports: [PerVideoScoreReport] = []
        var weightedSum = 0.0
        var weightSum = 0.0

        for metricSet in metricSets {
            guard let video = baseline.video(metricSet.stimulusId) else {
                reports.append(PerVideoScoreReport(
                    stimulusId: metricSet.stimulusId,
                    title: metricSet.stimulusId,
                    category: "Unattributed",
                    rows: [],
                    subscore: 0,
                    validSampleRatio: metricSet.validSampleRatio,
                    lowConfidence: metricSet.validSampleRatio < lowConfidenceValidRatio,
                    hasBaseline: false,
                    weight: 0
                ))
                continue
            }

            let report = scoreVideo(
                metricSet: metricSet,
                video: video,
                baseline: baseline,
                bracketId: bracketId,
                clipFrames: framesByClip[metricSet.stimulusId] ?? []
            )
            reports.append(report)
            weightedSum += video.weight * report.subscore
            weightSum += video.weight
        }

        let composite = weightSum > 0 ? weightedSum / weightSum : 0

        let formatter = ISO8601DateFormatter()
        return PerVideoSessionScore(
            ageBracketId: bracketId,
            ageMonths: ageMonths,
            compositeScore: composite,
            videos: reports,
            baselineStatus: baseline.status,
            baselineSchemaVersion: baseline.schemaVersion,
            scoredAtUtc: formatter.string(from: scoredAt)
        )
    }

    /// Bracket id for an age, clamped to the nearest declared bracket for out-of-range ages.
    static func resolvedBracketId(baseline: NormativeBaselineSet, ageMonths: Int) -> String {
        if let id = baseline.bracketId(forAgeMonths: ageMonths) {
            return id
        }
        guard let nearest = baseline.ageBrackets.min(by: {
            distance(ageMonths, to: $0) < distance(ageMonths, to: $1)
        }) else {
            return "default"
        }
        return nearest.id
    }

    private static func distance(_ ageMonths: Int, to bracket: NormativeBaselineSet.AgeBracket) -> Int {
        if ageMonths < bracket.minMonths { return bracket.minMonths - ageMonths }
        if ageMonths > bracket.maxMonths { return ageMonths - bracket.maxMonths }
        return 0
    }

    static func scoreVideo(
        metricSet: PerVideoMetricSet,
        video: NormativeVideo,
        baseline: NormativeBaselineSet,
        bracketId: String,
        clipFrames: [GazeFrame] = []
    ) -> PerVideoScoreReport {
        let clipDurationMs = max(metricSet.observedDurationSec, 0.5) * 1000.0
        var rows: [PerVideoScoreRow] = []

        for segment in video.segments {
            let lookup = metrics(for: segment, clipFrames: clipFrames, fallback: metricSet)
            for expectation in segment.expectations {
                let metricKey = composedKey(biomarker: expectation.biomarker, targetAOI: expectation.targetAOI)
                var observed = lookup[metricKey]
                var imputed = false
                if observed == nil, expectation.biomarker == PerVideoMetricKey.firstEntryLatencyMs.rawValue {
                    // Never fixated the target AOI: worst case for lower-is-better latency.
                    observed = clipDurationMs
                    imputed = true
                }
                guard let observedValue = observed else {
                    // Metric not produced for this clip and not imputable: skip rather than fabricate.
                    continue
                }
                guard let range = expectation.range(forBracketId: bracketId) else {
                    continue
                }

                let evaluation = evaluate(
                    observed: observedValue,
                    direction: expectation.direction,
                    range: range
                )

                rows.append(PerVideoScoreRow(
                    segmentId: segment.id,
                    segmentDescription: segment.description,
                    biomarker: expectation.biomarker,
                    targetAOI: expectation.targetAOI,
                    metricKey: metricKey,
                    direction: expectation.direction,
                    observed: observedValue,
                    expected: range.expected,
                    acceptableLow: range.acceptableLow,
                    acceptableHigh: range.acceptableHigh,
                    status: evaluation.status,
                    score: evaluation.score,
                    imputed: imputed,
                    citation: expectation.citation,
                    source: expectation.source
                ))
            }
        }

        let subscore = rows.isEmpty ? 0.0 : rows.reduce(0.0) { $0 + $1.score } / Double(rows.count)
        return PerVideoScoreReport(
            stimulusId: metricSet.stimulusId,
            title: video.title,
            category: video.category,
            rows: rows,
            subscore: subscore,
            validSampleRatio: metricSet.validSampleRatio,
            lowConfidence: metricSet.validSampleRatio < lowConfidenceValidRatio,
            hasBaseline: true,
            weight: video.weight
        )
    }

    /// Gaze and emotion numbers for one time section. Falls back to the whole-clip
    /// metric set when the section has no frames, so older callers stay stable.
    static func metrics(
        for segment: NormativeSegment,
        clipFrames: [GazeFrame],
        fallback: PerVideoMetricSet
    ) -> [String: Double] {
        guard !clipFrames.isEmpty else { return fallback.metrics }
        let window = clipFrames.filter { frame in
            let t = frame.gaze.clipMediaTimeSec ?? frame.gaze.videoTimeSec
            return t >= segment.startSec && t < segment.endSec
        }
        guard !window.isEmpty else { return fallback.metrics }
        return LegacyPerVideoMetricsExtractor.extractClip(
            stimulusId: fallback.stimulusId,
            frames: window,
            timeOriginSec: segment.startSec
        ).metrics
    }

    // MARK: Status and continuous score

    static func composedKey(biomarker: String, targetAOI: String?) -> String {
        guard let aoi = targetAOI, !aoi.isEmpty else { return biomarker }
        return "\(biomarker)::\(aoi)"
    }

    static func evaluate(observed: Double, direction: NormativeDirection, range: NormativeRange) -> (status: ExpectationStatus, score: Double) {
        switch direction {
        case .higherBetter:
            let expected = range.expected ?? range.acceptableLow ?? 0
            let low = range.acceptableLow ?? expected
            let status: ExpectationStatus
            if observed >= expected {
                status = .onTarget
            } else if observed >= low {
                status = .monitor
            } else {
                status = .atypical
            }
            return (status, piecewise(observed: observed, goodEnd: expected, badEnd: low - (expected - low)))

        case .lowerBetter:
            let expected = range.expected ?? range.acceptableHigh ?? 0
            let high = range.acceptableHigh ?? expected
            let status: ExpectationStatus
            if observed <= expected {
                status = .onTarget
            } else if observed <= high {
                status = .monitor
            } else {
                status = .atypical
            }
            return (status, piecewise(observed: observed, goodEnd: expected, badEnd: high + (high - expected)))

        case .band:
            guard let low = range.acceptableLow, let high = range.acceptableHigh, high > low else {
                let expected = range.expected ?? 0
                let status: ExpectationStatus = observed >= expected ? .onTarget : .monitor
                return (status, observed >= expected ? 1.0 : 0.5)
            }
            let center = (low + high) / 2.0
            let halfWidth = (high - low) / 2.0
            let distance = abs(observed - center)
            let status: ExpectationStatus
            if distance <= halfWidth {
                status = .onTarget
            } else if distance <= halfWidth * 2.0 {
                status = .monitor
            } else {
                status = .atypical
            }
            let score = max(0.0, min(1.0, 1.0 - max(0.0, distance - halfWidth) / halfWidth))
            return (status, score)
        }
    }

    /// Piecewise-linear goodness: 1 at `goodEnd`, 0 at `badEnd`, linear between and clamped beyond.
    static func piecewise(observed: Double, goodEnd: Double, badEnd: Double) -> Double {
        guard goodEnd != badEnd else { return observed >= goodEnd ? 1.0 : 0.0 }
        let t = (observed - badEnd) / (goodEnd - badEnd)
        return max(0.0, min(1.0, t))
    }
}
