import CurvAsdGazeKit
import PDFKit
import XCTest
@testable import AsdGazeCollector

final class DoctorReportGeneratorTests: XCTestCase {

    func testGazeSessionRecordRoundtripSerialization() throws {
        let record = GazeSessionRecord.fixture(
            sessionId: "SESS-12345",
            participantId: "child-SESS-123",
            durationSec: 30.0,
            socialPreferenceScore: 0.72,
            validFrameRatio: 0.94,
            eyeDwellRatio: 0.45,
            mouthDwellRatio: 0.20,
            firstEyeLatencyMs: 310.0,
            calibratedDistanceCm: 35.0,
            qualityVerdict: "optimal"
        )
        XCTAssertTrue(record.zeroRawVideoEnforced)
        XCTAssertEqual(record.socialPreferenceScore, 0.72)

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let data = try encoder.encode(record)

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let decoded = try decoder.decode(GazeSessionRecord.self, from: data)

        XCTAssertEqual(decoded.sessionId, record.sessionId)
        XCTAssertEqual(decoded.participantId, record.participantId)
        XCTAssertEqual(decoded.qualityVerdict, record.qualityVerdict)
    }

    func testGazeSessionRecordInitFromSummary() {
        let summary = GazeSessionReportSummary.fixture(
            sessionId: "SESS-SUMM-99",
            durationSec: 25.0,
            totalFrames: 1500,
            validFrames: 1420,
            validRatio: 0.946,
            socialPreferenceScore: -0.45,
            socialDwellSec: 5.5,
            geometricDwellSec: 14.5,
            eyeDwellRatio: 0.12,
            mouthDwellRatio: 0.08,
            qualityVerdict: .optimal
        )

        let record = GazeSessionRecord(summary: summary, participantToken: "child-SUMMARY")
        XCTAssertEqual(record.sessionId, "SESS-SUMM-99")
        XCTAssertEqual(record.participantId, "child-SUMMARY")
        XCTAssertEqual(record.captureSource, .trueDepth)
        XCTAssertEqual(record.socialPreferenceScore, -0.45)
        XCTAssertEqual(record.validFrameRatio, 0.946)
        XCTAssertEqual(record.qualityVerdict, "optimal")
    }

    func testDoctorReportPDFGenerationWithFullData() {
        let gazeRecord = GazeSessionRecord.fixture(
            sessionId: "SESS-PDF-FULL",
            participantId: "child-PDFFULL",
            startedAt: Date().addingTimeInterval(-60),
            endedAt: Date(),
            durationSec: 60.0,
            socialPreferenceScore: 0.65,
            validFrameRatio: 0.95,
            eyeDwellRatio: 0.48,
            mouthDwellRatio: 0.18,
            firstEyeLatencyMs: 340.0,
            calibratedDistanceCm: 36.0,
            qualityVerdict: "optimal"
        )

        let mchatResult = MCHATScoreResult(
            totalFailed: 1,
            riskLevel: .low,
            failedQuestionIds: [5],
            clinicalRecommendation: "Low risk profile."
        )

        let input = DoctorReportInput(
            participantId: "PEDIATRIC-CLINICAL-001",
            childAgeMonths: 28,
            guardianName: "Sarah Connor",
            assessmentDate: Date(),
            gazeRecord: gazeRecord,
            mchatResult: mchatResult,
            careKitCompletionRate: 92.0,
            careKitMedicationsLogged: 14,
            careKitDiaryEntriesLogged: 7
        )

        let pdfData = DoctorReportGenerator.generatePDFData(input: input)
        XCTAssertFalse(pdfData.isEmpty, "PDF data must not be empty")

        let pdfDoc = DoctorReportGenerator.generatePDFDocument(input: input)
        XCTAssertNotNil(pdfDoc, "Must generate a valid PDFKit PDFDocument")
        XCTAssertEqual(pdfDoc?.pageCount, 1, "Report must be single-page clinical summary")

        let page = pdfDoc?.page(at: 0)
        XCTAssertNotNil(page)
        let bounds = page?.bounds(for: .mediaBox) ?? .zero
        XCTAssertEqual(bounds.width, DoctorReportGenerator.pageWidth)
        XCTAssertEqual(bounds.height, DoctorReportGenerator.pageHeight)
    }

    func testDoctorReportPDFGenerationWithMinimalData() {
        let input = DoctorReportInput(
            participantId: "MINIMAL-PATIENT-002",
            childAgeMonths: 18,
            guardianName: "Guardian Only"
        )

        let pdfData = DoctorReportGenerator.generatePDFData(input: input)
        XCTAssertFalse(pdfData.isEmpty)

        let pdfDoc = DoctorReportGenerator.generatePDFDocument(input: input)
        XCTAssertNotNil(pdfDoc)
        XCTAssertEqual(pdfDoc?.pageCount, 1)
    }

    func testDoctorReportPDFGenerationWithHighRiskIndicators() {
        let gazeRecord = GazeSessionRecord.fixture(
            sessionId: "SESS-HIGH-RISK",
            participantId: "child-HIGHRISK",
            startedAt: Date().addingTimeInterval(-45),
            endedAt: Date(),
            durationSec: 45.0,
            socialPreferenceScore: -0.60,
            validFrameRatio: 0.88,
            eyeDwellRatio: 0.08,
            mouthDwellRatio: 0.05,
            firstEyeLatencyMs: 1200.0,
            calibratedDistanceCm: 32.0,
            qualityVerdict: "optimal"
        )

        let mchatResult = MCHATScoreResult(
            totalFailed: 9,
            riskLevel: .high,
            failedQuestionIds: [1, 5, 7, 9, 10, 12, 14, 15, 19],
            clinicalRecommendation: "Urgent specialist evaluation recommended."
        )

        let input = DoctorReportInput(
            participantId: "HIGH-RISK-003",
            childAgeMonths: 22,
            guardianName: "Alex Mercer",
            gazeRecord: gazeRecord,
            mchatResult: mchatResult
        )

        let pdfDoc = DoctorReportGenerator.generatePDFDocument(input: input)
        XCTAssertNotNil(pdfDoc)
        XCTAssertEqual(pdfDoc?.pageCount, 1)
    }

    // MARK: - Claims copy (CLM-3, CLM-4, CLM-5, SIM-5)

    private func pdfText(_ input: DoctorReportInput) throws -> String {
        let doc = try XCTUnwrap(DoctorReportGenerator.generatePDFDocument(input: input))
        return doc.string ?? ""
    }

    /// Eye, mouth and first-look metrics are absent (child never looked at a face region).
    private func makeSummary(source: CaptureSource) -> GazeSessionReportSummary {
        GazeSessionReportSummary.fixture(
            sessionId: "SESS-CLAIMS-01",
            captureSource: source,
            durationSec: 30,
            socialPreferenceScore: -0.45
        )
    }

    func testPDFContainsBothMandatorySentencesWithMinimalData() throws {
        let text = try pdfText(DoctorReportInput(participantId: "MIN-1"))
        XCTAssertTrue(text.contains("This is not a diagnostic tool."), text)
        XCTAssertTrue(text.contains("Part of Open Spectrum Tracking"), text)
    }

    func testPDFWithHighMChatResultNeverSaysElevatedRiskAndNamesInstrument() throws {
        let mchat = MCHATScoreResult(
            totalFailed: 9,
            riskLevel: .high,
            failedQuestionIds: [1, 5, 7],
            clinicalRecommendation: "Discuss with your pediatrician."
        )
        let input = DoctorReportInput(
            participantId: "HIGH-1",
            gazeRecord: GazeSessionRecord(summary: makeSummary(source: .trueDepth), participantToken: "child-SESS-CLA"),
            mchatResult: mchat
        )
        let text = try pdfText(input)

        XCTAssertTrue(text.contains("This is not a diagnostic tool."))
        XCTAssertTrue(text.contains("Part of Open Spectrum Tracking"))
        XCTAssertTrue(text.contains("M-CHAT-R/F:"), "instrument name must be present (CLM-4)")
        for forbidden in ["Elevated Risk", "High Risk", "Risk Stratification", "confidence", "IRB", "FDA", "Atypical"] {
            XCTAssertFalse(text.localizedCaseInsensitiveContains(forbidden), "PDF must not contain \(forbidden)")
        }
        XCTAssertFalse(text.contains("SIMULATED DATA"), "real captures carry no simulated banner")
    }

    func testPDFAbsentMetricsRenderAsNotObservedNotZero() throws {
        let input = DoctorReportInput(
            participantId: "NOEYE-1",
            gazeRecord: GazeSessionRecord(summary: makeSummary(source: .trueDepth), participantToken: "child-SESS-CLA")
        )
        let text = try pdfText(input)
        XCTAssertTrue(text.contains("Not observed"))
    }

    func testPDFRendersSimulatedBannerForSimulatedRecord() throws {
        let input = DoctorReportInput(
            participantId: "SIM-1",
            gazeRecord: GazeSessionRecord(summary: makeSummary(source: .simulated), participantToken: "child-SESS-CLA")
        )
        let text = try pdfText(input)
        XCTAssertTrue(text.contains("SIMULATED DATA"), text)
        XCTAssertTrue(text.contains("This is not a diagnostic tool."))
    }

    func testMChatLineAlwaysNamesInstrumentAndNextStep() {
        for level in [MCHATRiskLevel.low, .medium, .high] {
            let line = ClaimsCopy.mchatLine(for: level)
            XCTAssertTrue(line.hasPrefix("M-CHAT-R/F:"), line)
            XCTAssertTrue(line.contains("pediatrician"), line)
            XCTAssertNotEqual(line, level.rawValue, "bare persisted raw value must never be shown")
        }
    }

    // MARK: - Session History Store (data collection)

    @MainActor
    func testSessionHistoryPersistsAndRoundTrips() {
        let defaults = UserDefaults(suiteName: "GazeHistoryTests_\(UUID().uuidString)")!
        let store = GazeSessionHistoryStore(defaults: defaults)
        XCTAssertTrue(store.fetchAll().isEmpty)

        let record = GazeSessionRecord.fixture(
            sessionId: "SESS-HIST-1",
            participantId: "child-HIST1",
            startedAt: Date().addingTimeInterval(-60),
            endedAt: Date(),
            durationSec: 60.0,
            socialPreferenceScore: 0.5,
            validFrameRatio: 0.9,
            eyeDwellRatio: 0.4,
            mouthDwellRatio: 0.15
        )
        store.record(record)
        XCTAssertEqual(store.fetchAll().count, 1)
        XCTAssertEqual(store.lastRecord?.sessionId, "SESS-HIST-1")

        // A fresh store over the same defaults sees the persisted record.
        let reopened = GazeSessionHistoryStore(defaults: defaults)
        XCTAssertEqual(reopened.fetchAll().count, 1)
        XCTAssertEqual(reopened.lastRecord?.socialPreferenceScore, 0.5)
    }

    @MainActor
    func testSessionHistoryDedupesBySessionId() {
        let defaults = UserDefaults(suiteName: "GazeHistoryTests_\(UUID().uuidString)")!
        let store = GazeSessionHistoryStore(defaults: defaults)
        let base = Date()
        for score in [0.1, 0.9] {
            store.record(GazeSessionRecord.fixture(
                sessionId: "SESS-DUPE",
                participantId: "child-DUPE",
                startedAt: base.addingTimeInterval(-60),
                endedAt: base,
                durationSec: 60.0,
                socialPreferenceScore: score,
                validFrameRatio: 0.9,
                eyeDwellRatio: 0.4,
                mouthDwellRatio: 0.15
            ))
        }
        XCTAssertEqual(store.fetchAll().count, 1, "re-exports must not double-count")
        XCTAssertEqual(store.lastRecord?.socialPreferenceScore, 0.9, "latest write wins")
    }

    @MainActor
    func testSessionsThisWeekFiltersOldRecords() {
        let defaults = UserDefaults(suiteName: "GazeHistoryTests_\(UUID().uuidString)")!
        let store = GazeSessionHistoryStore(defaults: defaults)
        let now = Date()
        func record(id: String, ended: Date) {
            store.record(GazeSessionRecord.fixture(
                sessionId: id, participantId: "child-WEEK",
                startedAt: ended.addingTimeInterval(-60), endedAt: ended,
                durationSec: 60.0, socialPreferenceScore: 0.5,
                validFrameRatio: 0.9, eyeDwellRatio: 0.4, mouthDwellRatio: 0.15
            ))
        }
        record(id: "recent", ended: now.addingTimeInterval(-3600))
        record(id: "old", ended: now.addingTimeInterval(-10 * 24 * 3600))
        let week = store.sessionsThisWeek(now: now)
        XCTAssertEqual(week.count, 1)
        XCTAssertEqual(week.first?.sessionId, "recent")
    }
}
