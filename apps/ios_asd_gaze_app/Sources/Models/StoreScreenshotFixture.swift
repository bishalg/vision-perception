#if ASD_STORE_BUILD && ASD_STORE_SCREENSHOTS
import SwiftUI
import CurvAsdGazeKit

// MARK: - Store screenshot fixture (StoreSim only, WP-S.7)

/// App Store screenshots come from the real store UI on a simulator (`StoreSim` configuration:
/// `ASD_STORE_BUILD` + `ASD_STORE_SCREENSHOTS`). A simulator has no TrueDepth camera, so the
/// Maestro store flows (`maestro/flows/store/`) switch this fixture on with a launch argument:
///
///     -ASDStoreScreenshotFixture YES            capture entry treats the device as supported
///                                               with a first-run camera, and the Sessions tab
///                                               lists three example sessions
///     -ASDStoreScreenshotScreen result          also opens the post-session screen for one
///                                               example session
///
/// Nothing here is persisted: the example sessions live in memory for that launch only, are
/// marked `.simulated`, and carry no score values. This file compiles only where both flags
/// are set, which is `StoreSim` alone; `scripts/check_release_symbols.sh` asserts the launch
/// key is absent from Release and DemoRelease.
enum StoreScreenshotFixture {
    static let activeKey = "ASDStoreScreenshotFixture"
    static let screenKey = "ASDStoreScreenshotScreen"

    /// `true` only when this launch carries `-ASDStoreScreenshotFixture YES` (argument domain).
    static var isActive: Bool {
        UserDefaults.standard.bool(forKey: activeKey)
    }

    static var opensResultScreen: Bool {
        isActive && UserDefaults.standard.string(forKey: screenKey) == "result"
    }

    /// Example session for the post-session screen: recording quality only, no score values.
    static func summary(durationSec: Double = 96, validRatio: Double = 0.93) -> GazeSessionReportSummary {
        let total = Int(durationSec * 60)
        let valid = Int(Double(total) * validRatio)
        return GazeSessionReportSummary(
            sessionId: "3f9c2a71-5b8e-4d0a-9c6e-2b7d1e4f8a10",
            captureSource: .simulated,
            durationSec: durationSec,
            totalFrames: total,
            validFrames: valid,
            validRatio: validRatio,
            socialPreferenceScore: nil,
            socialDwellSec: 0,
            geometricDwellSec: 0,
            eyeDwellRatio: nil,
            mouthDwellRatio: nil,
            firstEyeLatencyMs: nil,
            socialSmileIntensityMean: nil,
            smileReactivityIndex: nil,
            flatAffectScore: nil,
            surpriseScoreMean: nil,
            qualityVerdict: .optimal,
            subjectAgeSignal: nil,
            interruptionCount: 0,
            interruptedSec: 0
        )
    }

    /// A smooth synthetic scanpath (Lissajous curve) for the replay hero. Geometry only.
    static func frames(count: Int = 600) -> [GazeFrame] {
        (0..<count).map { i in
            let t = Double(i) / 60.0
            let x = 0.5 + 0.32 * sin(t * 0.9) * cos(t * 0.23)
            let y = 0.5 + 0.28 * sin(t * 1.3 + 0.6)
            return GazeFrame(
                gaze: GazeTelemetrySample(timestampMs: t * 1000, videoTimeSec: t, xNorm: x, yNorm: y, confidence: 0.95)
            )
        }
    }

    /// Three example sessions for the Sessions tab, newest first after sorting.
    static func sessionRecords(now: Date = Date()) -> [GazeSessionRecord] {
        let examples: [(daysAgo: Double, durationSec: Double, validRatio: Double, verdict: String, clips: Int)] = [
            (9, 92, 0.88, "acceptable", 5),
            (4, 104, 0.91, "optimal", 6),
            (0.1, 96, 0.93, "optimal", 6)
        ]
        return examples.enumerated().map { index, item in
            let end = now.addingTimeInterval(-item.daysAgo * 24 * 3600)
            let sessionId = "screenshot-example-\(index + 1)"
            return GazeSessionRecord(
                sessionId: sessionId,
                participantId: "child-example",
                captureSource: .simulated,
                startedAt: end.addingTimeInterval(-item.durationSec),
                endedAt: end,
                durationSec: item.durationSec,
                socialPreferenceScore: nil,
                validFrameRatio: item.validRatio,
                eyeDwellRatio: nil,
                mouthDwellRatio: nil,
                firstEyeLatencyMs: nil,
                calibratedDistanceCm: nil,
                qualityVerdict: item.verdict,
                socialSmileIntensity: nil,
                smileReactivityIndex: nil,
                flatAffectScore: nil,
                surpriseScoreMean: nil,
                perVideo: (0..<item.clips).map { clip in
                    PerVideoScoreReport(
                        stimulusId: "example_clip_\(clip + 1)",
                        title: "Clip \(clip + 1)",
                        category: "example",
                        rows: [],
                        subscore: 0,
                        validSampleRatio: item.validRatio,
                        lowConfidence: false,
                        hasBaseline: false,
                        weight: 1
                    )
                }
            )
        }
    }
}

/// The simulator grants camera access without asking, so the fixture reports a first-run
/// camera (iOS has not asked yet). That puts the in-app camera primer (S-11) in front of the
/// system prompt exactly as on a new device; a request is declined, so no capture starts.
@MainActor
final class StoreScreenshotCameraAuthorizer: CameraAuthorizing {
    var status: CameraAuthorizationStatus { .notDetermined }
    func requestAccess() async -> Bool { false }
}

/// Applied to the app root in `StoreSim` only: shows the example sessions and, when asked,
/// the post-session screen. Inactive unless the launch argument is present.
struct StoreScreenshotFixtureModifier: ViewModifier {
    @State private var showResult = false

    func body(content: Content) -> some View {
        content
            .onAppear {
                guard StoreScreenshotFixture.isActive else { return }
                GazeSessionHistoryStore.shared.showScreenshotExamples(StoreScreenshotFixture.sessionRecords())
                showResult = StoreScreenshotFixture.opensResultScreen
            }
            .fullScreenCover(isPresented: $showResult) {
                GazeSessionResultView(
                    summary: StoreScreenshotFixture.summary(),
                    recordedFrames: StoreScreenshotFixture.frames(),
                    onDismiss: { showResult = false },
                    onRetake: { showResult = false }
                )
            }
    }
}
#endif
