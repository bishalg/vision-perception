import CurvAsdGazeKit
import Foundation
@testable import AsdGazeCollector

/// Test-only construction helpers for the app target.
///
/// Production initialisers of `GazeSessionReportSummary` and `GazeSessionRecord`
/// require every metric and the capture source explicitly (PLC-1, SIM-4); the
/// defaults live here, in the test target, so no placeholder value can leak into
/// shipped code. Optional metrics default to `nil` (absent), never to a constant.
extension GazeSessionReportSummary {
    static func fixture(
        sessionId: String = "TEST0000",
        captureSource: CaptureSource = .trueDepth,
        durationSec: Double = 15.0,
        totalFrames: Int = 900,
        validFrames: Int = 850,
        validRatio: Double = 0.94,
        socialPreferenceScore: Double = 0.0,
        socialDwellSec: Double = 10.0,
        geometricDwellSec: Double = 5.0,
        eyeDwellRatio: Double? = nil,
        mouthDwellRatio: Double? = nil,
        firstEyeLatencyMs: Double? = nil,
        socialSmileIntensityMean: Double = 0.3,
        smileReactivityIndex: Double = 0.3,
        flatAffectScore: Double = 0.2,
        surpriseScoreMean: Double = 0.2,
        qualityVerdict: GazeQualityGate.Verdict = .optimal,
        subjectAgeSignal: SubjectAgeSignal? = nil,
        interruptionCount: Int = 0,
        interruptedSec: Double = 0.0
    ) -> GazeSessionReportSummary {
        GazeSessionReportSummary(
            sessionId: sessionId,
            captureSource: captureSource,
            durationSec: durationSec,
            totalFrames: totalFrames,
            validFrames: validFrames,
            validRatio: validRatio,
            socialPreferenceScore: socialPreferenceScore,
            socialDwellSec: socialDwellSec,
            geometricDwellSec: geometricDwellSec,
            eyeDwellRatio: eyeDwellRatio,
            mouthDwellRatio: mouthDwellRatio,
            firstEyeLatencyMs: firstEyeLatencyMs,
            socialSmileIntensityMean: socialSmileIntensityMean,
            smileReactivityIndex: smileReactivityIndex,
            flatAffectScore: flatAffectScore,
            surpriseScoreMean: surpriseScoreMean,
            qualityVerdict: qualityVerdict,
            subjectAgeSignal: subjectAgeSignal,
            interruptionCount: interruptionCount,
            interruptedSec: interruptedSec
        )
    }
}

extension GazeSessionRecord {
    static func fixture(
        id: String = UUID().uuidString,
        sessionId: String = "TEST0000",
        participantId: String = "child-TEST0000",
        captureSource: CaptureSource = .trueDepth,
        startedAt: Date = Date(),
        endedAt: Date? = nil,
        durationSec: Double = 30.0,
        socialPreferenceScore: Double = 0.5,
        validFrameRatio: Double = 0.94,
        eyeDwellRatio: Double? = nil,
        mouthDwellRatio: Double? = nil,
        firstEyeLatencyMs: Double? = nil,
        calibratedDistanceCm: Double = 40.0,
        qualityVerdict: String = "optimal",
        socialSmileIntensity: Double = 0.3,
        smileReactivityIndex: Double = 0.3,
        flatAffectScore: Double = 0.2,
        surpriseScoreMean: Double = 0.2,
        notes: String? = nil,
        schemaVersion: String = GazeSessionRecord.currentSchemaVersion,
        legacyProvenance: Bool? = nil,
        perVideoComposite: Double? = nil,
        perVideo: [PerVideoScoreReport]? = nil
    ) -> GazeSessionRecord {
        GazeSessionRecord(
            id: id,
            sessionId: sessionId,
            participantId: participantId,
            captureSource: captureSource,
            startedAt: startedAt,
            endedAt: endedAt ?? startedAt.addingTimeInterval(durationSec),
            durationSec: durationSec,
            socialPreferenceScore: socialPreferenceScore,
            validFrameRatio: validFrameRatio,
            eyeDwellRatio: eyeDwellRatio,
            mouthDwellRatio: mouthDwellRatio,
            firstEyeLatencyMs: firstEyeLatencyMs,
            calibratedDistanceCm: calibratedDistanceCm,
            qualityVerdict: qualityVerdict,
            socialSmileIntensity: socialSmileIntensity,
            smileReactivityIndex: smileReactivityIndex,
            flatAffectScore: flatAffectScore,
            surpriseScoreMean: surpriseScoreMean,
            notes: notes,
            schemaVersion: schemaVersion,
            legacyProvenance: legacyProvenance,
            perVideoComposite: perVideoComposite,
            perVideo: perVideo
        )
    }
}
