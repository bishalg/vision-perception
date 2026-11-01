import SwiftUI
import AsdCore
import CurvAsdGazeKit

/// DRF-1: one check target between clips and after the last clip, at the position the
/// pipeline gives (`drift.checkLayout`, cycling). The pipeline measures the offset and drift
/// (DRF-2, DRF-3); this view only shows the ladybug and reports the advice read after it.
struct DriftCheckTargetView: View {
    @ObservedObject var controller: GazeSessionController
    /// Advice read right after the check (act on it only here, at the clip boundary, CAL-9).
    let onFinished: (GazeRecalAdviceRecord?) -> Void
    @StateObject private var stage = GazeTargetStage()

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            GazeTargetStageLayer(stage: stage)
            VStack {
                Spacer()
                TargetHintPill(text: Localizer.tr("target_check_hint"), progress: nil)
                    .padding(.bottom, 40)
            }
        }
        .ignoresSafeArea()
        .accessibilityIdentifier("drift_check_step")
        .task { await run() }
    }

    private func run() async {
        guard let schedule = await controller.targetSchedule() else {
            // No live pipeline (Debug simulation): nothing to measure.
            onFinished(nil)
            return
        }
        do {
            _ = try await stage.runner(for: controller, schedule: schedule).runDriftCheck()
        } catch is CancellationError {
            stage.clear()
            return
        } catch {
            onFinished(nil)
            return
        }
        onFinished(await controller.recalibrationAdvice())
    }
}

/// CAL-9 prompt at a clip boundary: re-calibrate (with validation) before the next clip, or
/// keep going on the calibration in effect.
struct RecalibrationPromptView: View {
    let reasons: [GazeRecalReason]
    let onRecalibrate: () -> Void
    let onContinue: () -> Void

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            OperatorCard(
                icon: "scope",
                title: Localizer.tr("recal_prompt_title"),
                message: ([Localizer.tr("recal_prompt_body")] + reasons.map { RecalibrationReasonText.text(for: $0) })
                    .joined(separator: "\n"),
                primary: (Localizer.tr("recal_prompt_accept"), "recal_prompt_accept", onRecalibrate),
                secondary: (Localizer.tr("recal_prompt_decline"), "recal_prompt_decline", onContinue)
            )
        }
        .ignoresSafeArea()
    }
}
