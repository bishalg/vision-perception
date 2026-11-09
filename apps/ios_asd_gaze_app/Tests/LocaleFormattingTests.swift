@testable import AsdGazeCollector
import AsdCore
import CurvAsdGazeKit
import XCTest

/// WP-LN.3 (I-5): user-visible numbers follow the user's locale; machine output does not.
@MainActor
final class LocaleFormattingTests: XCTestCase {
    func testUserVisibleOffsetUsesTheLocaleDecimalSeparator() {
        let advice = GazeRecalAdviceRecord(action: .recalibrate, reasons: [.validationAccuracy], limitReached: false)
        let de = ValidationResultCard(
            advice: advice, accuracyDeg: 5.62, canRetry: true, onRetry: {}, onContinue: {},
            formatLocale: Locale(identifier: "de_DE"))
        XCTAssertTrue(de.message.contains("5,6"), de.message)
        XCTAssertFalse(de.message.contains("5.6"), de.message)

        let us = ValidationResultCard(
            advice: advice, accuracyDeg: 5.62, canRetry: true, onRetry: {}, onContinue: {},
            formatLocale: Locale(identifier: "en_US"))
        XCTAssertTrue(us.message.contains("5.6"), us.message)
    }

    func testMachineOutputStaysDottedWhateverTheUserLocale() {
        XCTAssertEqual(String(format: "%.5f", locale: FormatLocale.posix, 0.5), "0.50000")
        #if ASD_ML_DATA_EXPORT
        let logger = BlendShapeCSVLogger(
            directory: FileManager.default.temporaryDirectory.appendingPathComponent("ln3-csv"),
            isNormCollectionOptedIn: { false })
        let key = FaceBlendShapeCSV.columns[0]
        let line = logger.csvLine(timestamp: Date(timeIntervalSince1970: 0), sessionId: "s", isRecording: true, shapes: [key: 0.25])
        XCTAssertTrue(line.contains("0.25000"), line)
        XCTAssertFalse(line.contains("0,25000"), line)
        #endif
    }
}
