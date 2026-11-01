@testable import AsdGazeCollector
import AsdCore
import CurvAsdGazeKit
import XCTest

/// WP-G.10: the app screens around the pipeline's calibration, validation and drift checks
/// (CAL-7 .. CAL-9, VAL-1, DRF-1). The flow state machines themselves are tested in the kit
/// (`GazeTargetFlowTests`, including a full synthetic session through the Rust pipeline).
@MainActor
final class GazeCalibrationScreenTests: XCTestCase {
    // MARK: Wizard order

    func testWizardRunsCalibrationAndValidationBeforeTheBattery() {
        XCTAssertEqual(ScreeningWizardStep.captureOrder, [.positioning, .gate, .calibrating, .stimulus])
        XCTAssertEqual(ScreeningWizardStep.positioning.nextCaptureStep, .gate)
        XCTAssertEqual(ScreeningWizardStep.gate.nextCaptureStep, .calibrating,
                       "stimuli are on the device before calibrating, so the calibration is fresh for the battery")
        XCTAssertEqual(ScreeningWizardStep.calibrating.nextCaptureStep, .stimulus)
        XCTAssertNil(ScreeningWizardStep.stimulus.nextCaptureStep)

        // Inside the calibrating step: calibration, then validation, then the battery.
        var flow = GazeCalibrationFlow()
        XCTAssertEqual(flow.phase, .calibrating(reason: nil))
        flow.calibrationFinished(.fair, advice: nil)
        XCTAssertEqual(flow.phase, .validating)
        flow.validationFinished(advice: nil, accuracyDeg: nil)
        XCTAssertTrue(flow.isReady)
    }

    // MARK: Targets

    func testValidationAndChecksUseOtherCharactersThanCalibration() {
        XCTAssertEqual(TargetCharacter.forKind(.calibration), .balloon)
        XCTAssertNotEqual(TargetCharacter.forKind(.validation), TargetCharacter.forKind(.calibration), "VAL-1")
        XCTAssertNotEqual(TargetCharacter.forKind(.driftCheck), TargetCharacter.forKind(.calibration))
    }

    func testTargetShrinksFromLargeToASmallBrightCentre() {
        XCTAssertEqual(ShrinkingTargetView.size(progress: 0), ShrinkingTargetView.startSize)
        XCTAssertEqual(ShrinkingTargetView.size(progress: 1), ShrinkingTargetView.endSize)
        XCTAssertEqual(ShrinkingTargetView.size(progress: 3), ShrinkingTargetView.endSize, "holds small while the pipeline extends the window")
        var last = CGFloat.infinity
        for i in 0...20 {
            let s = ShrinkingTargetView.size(progress: Double(i) / 20)
            XCTAssertLessThanOrEqual(s, last)
            last = s
        }
        // CAL-8: the centre is at most 1 degree at 45 cm (7.85 mm). Points are at least
        // 1/150 inch on every supported device, so this bound is conservative.
        let mmPerPoint = 25.4 / 150
        let oneDegreeAt45cmMm = tan(Double.pi / 180) * 450
        XCTAssertLessThan(Double(ShrinkingTargetView.centreSize) * mmPerPoint, oneDegreeAt45cmMm)
        XCTAssertLessThan(Double(ShrinkingTargetView.endSize) * mmPerPoint, oneDegreeAt45cmMm)
    }

    func testStageShowsATargetAndClearsItAfterThePop() async {
        let stage = GazeTargetStage(soundEnabled: false, popDuration: 0, gapDuration: 0)
        let p = GazeTargetPresentation(kind: .calibration, target: GazeVec2(x: 0.12, y: 0.88), index: 3, count: 5, attempt: 1, windowSec: 1.6)
        stage.show(p)
        XCTAssertEqual(stage.current, p)
        XCTAssertFalse(stage.popping)
        XCTAssertEqual(stage.shownCount, 1)
        await stage.ended(p, result: nil)
        XCTAssertNil(stage.current, "a re-show or miss fades without a pop")
        XCTAssertFalse(stage.popping)
    }

    // MARK: Operator text

    func testValidationCardStatesTheMeasuredOffsetAndReasons() {
        let advice = GazeRecalAdviceRecord(action: .recalibrate, reasons: [.validationAccuracy], limitReached: false)
        let card = ValidationResultCard(advice: advice, accuracyDeg: 5.62, canRetry: true, onRetry: {}, onContinue: {})
        XCTAssertTrue(card.message.contains("5.6"), card.message)
        XCTAssertTrue(card.message.contains(RecalibrationReasonText.text(for: .validationAccuracy)))
        let missed = ValidationResultCard(advice: advice, accuracyDeg: nil, canRetry: false, onRetry: {}, onContinue: {})
        XCTAssertTrue(missed.message.contains(Localizer.tr("validation_result_missed")))
    }

    static let newKeys = [
        "target_calibration_hint", "target_validation_hint", "target_check_hint",
        "calibrate_result_great", "calibrate_result_poor_title", "calibrate_result_poor_body",
        "calibrate_failed_title", "calibrate_failed_body", "calibrate_try_again",
        "calibrate_continue_anyway", "calibrate_limit_reached",
        "validation_result_recal_title", "validation_result_warn_title", "validation_result_offset",
        "validation_result_missed", "validation_calibrate_again", "validation_continue",
        "recal_prompt_title", "recal_prompt_body", "recal_prompt_accept", "recal_prompt_decline"
    ]

    func testEveryCalibrationStringExistsInEveryLanguage() {
        let reasons: [GazeRecalReason] = [.calibrationQuality, .validationAccuracy, .drift, .poseShift, .orientationChanged, .calibrationAge, .operatorRequest]
        let keys = Self.newKeys + reasons.map(RecalibrationReasonText.key(for:))
        for key in keys {
            let en = Localizer.tr(key, lang: .english)
            XCTAssertNotEqual(en, key, "\(key) missing in en")
            for locale in [AppLocale.nepali, .hindi] {
                let text = Localizer.tr(key, lang: locale)
                XCTAssertNotEqual(text, key, "\(key) missing in \(locale)")
                XCTAssertNotEqual(text, en, "\(key) not translated to \(locale)")
            }
        }
        for locale in AppLocale.allCases {
            XCTAssertTrue(Localizer.tr("validation_result_offset", lang: locale).contains("%.1f"), "\(locale) format")
        }
    }
}
