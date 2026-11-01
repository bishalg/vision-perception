import SwiftUI
import AsdCore
import CurvAsdGazeKit

/// VAL-1: after every calibration, the validation layout (default four targets, not at
/// calibration positions) with a different character (a star), run through the session's
/// Rust pipeline. Accuracy, completeness and the advice are the pipeline's (VAL-2 .. VAL-4).
struct ValidationStepView: View {
    @ObservedObject var controller: GazeSessionController
    /// Advice read right after the last target, and the validation accuracy in degrees.
    let onFinished: (GazeRecalAdviceRecord?, Double?) -> Void
    @StateObject private var stage = GazeTargetStage()

    var body: some View {
        ZStack {
            GazeTargetStageLayer(stage: stage)
            VStack {
                Spacer()
                TargetHintPill(
                    text: Localizer.tr("target_validation_hint"),
                    progress: stage.current.map { "\($0.index + 1) / \($0.count)" }
                )
                .padding(.bottom, 40)
            }
        }
        .accessibilityIdentifier("validation_step")
        .task { await run() }
    }

    private func run() async {
        guard let schedule = await controller.targetSchedule() else {
            onFinished(nil, nil)
            return
        }
        do {
            _ = try await stage.runner(for: controller, schedule: schedule).runValidation()
        } catch is CancellationError {
            stage.clear()
            return
        } catch {
            // No pipeline: nothing was measured; the advice below says what that means.
        }
        let summary = await controller.validationSummary()
        onFinished(await controller.recalibrationAdvice(), summary?.accuracyDeg)
    }
}

/// Validation advice for the operator (VAL-4): above the warning level the operator may go on;
/// above the limit (or with no target hit) re-calibration is advised.
struct ValidationResultCard: View {
    let advice: GazeRecalAdviceRecord
    let accuracyDeg: Double?
    let canRetry: Bool
    let onRetry: () -> Void
    let onContinue: () -> Void

    private var isRecalibrate: Bool { advice.action == .recalibrate }

    var message: String {
        var lines: [String] = []
        if let accuracyDeg {
            lines.append(String(format: Localizer.tr("validation_result_offset"), accuracyDeg))
        } else {
            lines.append(Localizer.tr("validation_result_missed"))
        }
        lines.append(contentsOf: advice.reasons.map { RecalibrationReasonText.text(for: $0) })
        return lines.joined(separator: "\n")
    }

    var body: some View {
        OperatorCard(
            icon: isRecalibrate ? "exclamationmark.triangle.fill" : "info.circle.fill",
            title: Localizer.tr(isRecalibrate ? "validation_result_recal_title" : "validation_result_warn_title"),
            message: message,
            note: canRetry ? nil : Localizer.tr("calibrate_limit_reached"),
            primary: isRecalibrate
                ? (canRetry ? (Localizer.tr("validation_calibrate_again"), "validation_calibrate_again", onRetry) : nil)
                : (Localizer.tr("validation_continue"), "validation_continue", onContinue),
            secondary: isRecalibrate
                ? (Localizer.tr("calibrate_continue_anyway"), "validation_continue_anyway", onContinue)
                : (canRetry ? (Localizer.tr("validation_calibrate_again"), "validation_calibrate_again", onRetry) : nil)
        )
    }
}

/// Operator text for each re-calibration reason (CAL-9).
enum RecalibrationReasonText {
    static func key(for reason: GazeRecalReason) -> String {
        switch reason {
        case .calibrationQuality: return "recal_reason_quality"
        case .validationAccuracy: return "recal_reason_validation"
        case .drift: return "recal_reason_drift"
        case .poseShift: return "recal_reason_pose"
        case .orientationChanged: return "recal_reason_orientation"
        case .calibrationAge: return "recal_reason_age"
        case .operatorRequest: return "recal_reason_operator"
        }
    }

    static func text(for reason: GazeRecalReason, lang: AppLocale? = nil) -> String {
        Localizer.tr(key(for: reason), lang: lang)
    }
}
