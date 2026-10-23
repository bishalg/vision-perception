import Foundation
@testable import CurvAsdGazeKit

/// Test-only construction helper for `GazeSessionReportSummary`.
///
/// Production initialisers require every metric explicitly (PLC-1, SIM-4); the
/// defaults live here, in the test target, so no placeholder value can leak into
/// shipped code. Optional metrics default to `nil` (absent), never to a constant.
extension GazeSessionReportSummary {
    static func fixture(
        sessionId: String = "TEST0000",
        captureSource: CaptureSource = .trueDepth,
        durationSec: Double = 10.0,
        totalFrames: Int = 600,
        validFrames: Int = 570,
        validRatio: Double = 0.95,
        socialPreferenceScore: Double = 0.0,
        socialDwellSec: Double = 5.0,
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

extension SessionHeader {
    static func fixture(
        sessionId: String = "TEST0000",
        participantToken: String = "child-TEST0000",
        captureSource: CaptureSource = .trueDepth,
        stimulusDurationSec: Double = 10.0
    ) -> SessionHeader {
        SessionHeader(
            sessionId: sessionId,
            participantToken: participantToken,
            stimulusBattery: "GeoPref_v1",
            stimulusDurationSec: stimulusDurationSec,
            captureSource: captureSource
        )
    }
}
