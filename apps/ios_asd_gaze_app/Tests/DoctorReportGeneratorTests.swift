import CurvAsdGazeKit
import PDFKit
import XCTest
@testable import AsdGazeCollector

final class DoctorReportGeneratorTests: XCTestCase {

    func testGazeSessionRecordRoundtripSerialization() throws {
        let record = GazeSessionRecord(
            sessionId: "SESS-12345",
            participantId: "STUDY-TEST-01",
            startedAt: Date(),
            endedAt: Date().addingTimeInterval(30),
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
        let summary = GazeSessionReportSummary(
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

        let record = GazeSessionRecord(summary: summary, participantId: "STUDY-PART-SUMMARY")
        XCTAssertEqual(record.sessionId, "SESS-SUMM-99")
        XCTAssertEqual(record.participantId, "STUDY-PART-SUMMARY")
        XCTAssertEqual(record.socialPreferenceScore, -0.45)
        XCTAssertEqual(record.validFrameRatio, 0.946)
        XCTAssertEqual(record.qualityVerdict, "optimal")
    }

    func testDoctorReportPDFGenerationWithFullData() {
        let gazeRecord = GazeSessionRecord(
            sessionId: "SESS-PDF-FULL",
            participantId: "PEDIATRIC-CLINICAL-001",
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
        let gazeRecord = GazeSessionRecord(
            sessionId: "SESS-HIGH-RISK",
            participantId: "HIGH-RISK-003",
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

    // MARK: - Session History Store (data collection)

    @MainActor
    func testSessionHistoryPersistsAndRoundTrips() {
        let defaults = UserDefaults(suiteName: "GazeHistoryTests_\(UUID().uuidString)")!
        let store = GazeSessionHistoryStore(defaults: defaults)
        XCTAssertTrue(store.fetchAll().isEmpty)

        let record = GazeSessionRecord(
            sessionId: "SESS-HIST-1",
            participantId: "P-1",
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
            store.record(GazeSessionRecord(
                sessionId: "SESS-DUPE",
                participantId: "P-1",
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
            store.record(GazeSessionRecord(
                sessionId: id, participantId: "P-1",
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
