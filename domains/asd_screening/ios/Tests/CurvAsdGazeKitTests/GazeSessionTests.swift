import XCTest
@testable import CurvAsdGazeKit

final class GazeSessionTests: XCTestCase {
    func testSessionControllerSimulationRun() {
        let controller = GazeSessionController()
        controller.startSession()
        XCTAssertTrue(controller.isRecording)

        // Simulate 30 frames at 60 Hz (0.5 seconds)
        for i in 0..<30 {
            let t = Double(i) / 60.0
            controller.tickSimulatedFrame(timeSec: t)
        }

        let summary = controller.stopSession()
        XCTAssertFalse(controller.isRecording)
        XCTAssertEqual(summary.totalFrames, 30)
        XCTAssertGreaterThan(summary.validRatio, 0.8)
        XCTAssertGreaterThanOrEqual(summary.socialPreferenceScore, -1.0)
        XCTAssertLessThanOrEqual(summary.socialPreferenceScore, 1.0)
    }

    func testVaultEncryptionAndDecryption() throws {
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let vault = SessionVaultManager(vaultDirectory: tempDir)

        let header = SessionHeader(
            sessionId: "TEST1234",
            participantToken: "child-test-token",
            stimulusBattery: "GeoPref_v1",
            stimulusDurationSec: 10.0
        )

        let sample = GazeTelemetrySample(
            timestampMs: 100.0,
            xNorm: 0.35,
            yNorm: 0.45,
            confidence: 0.95
        )
        let frame = GazeFrame(
            gaze: sample,
            affect: .neutral,
            activeAOINames: ["Social_Actor"],
            aoiDwellMs: 100.0,
            qualityVerdict: .optimal
        )

        let report = GazeSessionReportSummary(
            sessionId: "TEST1234",
            durationSec: 10.0,
            totalFrames: 1,
            validFrames: 1,
            validRatio: 1.0,
            socialPreferenceScore: 0.55,
            socialDwellSec: 6.0,
            geometricDwellSec: 4.0
        )

        let fileURL = try vault.saveSession(header: header, frames: [frame], report: report)
        XCTAssertTrue(FileManager.default.fileExists(atPath: fileURL.path))

        let decryptedText = try vault.readSessionPlainText(from: fileURL)
        XCTAssertTrue(decryptedText.contains("TEST1234"))
        XCTAssertTrue(decryptedText.contains("Social_Actor"))
        XCTAssertTrue(decryptedText.contains("socialPreferenceScore"))

        try? FileManager.default.removeItem(at: tempDir)
    }

    func testVaultKeyPersistsAcrossManagerInstances() throws {
        // Mirrors a relaunch: a manager created later must decrypt sessions written
        // by an earlier one. Skipped when the Keychain is unavailable in this
        // environment (both instances would then hold unrelated ephemeral keys).
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let first = SessionVaultManager(vaultDirectory: tempDir)
        try XCTSkipUnless(first.isKeyPersisted, "Keychain unavailable — key persistence not testable here")

        let header = SessionHeader(
            sessionId: "PERSIST1",
            participantToken: "child-test-token",
            stimulusBattery: "GeoPref_v1",
            stimulusDurationSec: 1.0
        )
        let frame = GazeFrame(
            gaze: GazeTelemetrySample(timestampMs: 0.0, xNorm: 0.5, yNorm: 0.5, confidence: 0.9),
            affect: .neutral,
            activeAOINames: [],
            aoiDwellMs: 16.7,
            qualityVerdict: .optimal
        )
        let report = GazeSessionReportSummary(
            sessionId: "PERSIST1",
            durationSec: 1.0,
            totalFrames: 1,
            validFrames: 1,
            validRatio: 1.0,
            socialPreferenceScore: 0.0,
            socialDwellSec: 0.5,
            geometricDwellSec: 0.5
        )
        let fileURL = try first.saveSession(header: header, frames: [frame], report: report)

        let second = SessionVaultManager(vaultDirectory: tempDir)
        XCTAssertTrue(second.isKeyPersisted)
        let decryptedText = try second.readSessionPlainText(from: fileURL)
        XCTAssertTrue(decryptedText.contains("PERSIST1"))
    }

    func testVaultStoresSubjectAgeSignal() throws {
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let vault = SessionVaultManager(vaultDirectory: tempDir)
        let header = SessionHeader(
            sessionId: "AGESIG1",
            participantToken: "child-test-token",
            stimulusBattery: "GeoPref_v1",
            stimulusDurationSec: 5.0
        )
        let report = GazeSessionReportSummary(
            sessionId: "AGESIG1",
            durationSec: 5.0,
            totalFrames: 300,
            validFrames: 280,
            validRatio: 0.933,
            socialPreferenceScore: 0.4,
            socialDwellSec: 3.0,
            geometricDwellSec: 2.0,
            subjectAgeSignal: SubjectAgeSignal(
                band: .toddler,
                agreement: 1.0,
                confidence: 0.82,
                sampleCount: 12,
                isStable: true
            )
        )

        let fileURL = try vault.saveSession(header: header, frames: [], report: report)
        let decryptedText = try vault.readSessionPlainText(from: fileURL)
        XCTAssertTrue(decryptedText.contains("subjectAgeSignal"))
        XCTAssertTrue(decryptedText.contains("toddler"))
    }

    func testExportSchemas() {
        let report = GazeSessionReportSummary(
            sessionId: "EXP789",
            durationSec: 30.0,
            totalFrames: 1800,
            validFrames: 1700,
            validRatio: 0.944,
            socialPreferenceScore: 0.48,
            socialDwellSec: 18.0,
            geometricDwellSec: 12.0
        )

        let rk = SessionVaultManager.makeResearchKitJson(report: report)
        XCTAssertEqual(rk["identifier"] as? String, "ORKActiveStepResult")
        XCTAssertEqual(rk["sessionId"] as? String, "EXP789")
        XCTAssertEqual(rk["isValid"] as? Bool, true)

        let ck = SessionVaultManager.makeCareKitJson(report: report)
        XCTAssertEqual(ck["identifier"] as? String, "OCKCarePlanActivity")
        XCTAssertEqual(ck["sessionId"] as? String, "EXP789")
    }
}
