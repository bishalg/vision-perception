import XCTest
@testable import CurvAsdGazeKit

final class GazeSessionTests: XCTestCase {
    // MARK: - SIM-2: live capture never falls back to simulation

    #if os(macOS)
    @MainActor
    func testStartSessionWithoutFaceTrackingIsUnavailable() async {
        // macOS has no ARFaceTrackingConfiguration; the live path must refuse to
        // record instead of silently simulating.
        let controller = GazeSessionController()
        controller.startSession()
        XCTAssertFalse(controller.isRecording)
        XCTAssertNil(controller.captureSource)
        XCTAssertFalse(controller.isSimulationMode)
        XCTAssertEqual(controller.trackingStage, .unavailable(reason: .faceTrackingUnsupported))

        // Simulated ticks after a refused live start record nothing.
        controller.tickSimulatedFrame(timeSec: 0.1)
        let summary = await controller.stopSession()
        XCTAssertEqual(summary.totalFrames, 0)
        XCTAssertEqual(summary.qualityVerdict, .rejected)
        XCTAssertNotEqual(summary.captureSource, .trueDepth, "A refused live start must not claim TrueDepth data")
    }
    #endif

    // MARK: - SIM-3: simulation only through the explicit, Debug-only entry point

    @MainActor
    func testSimulatedSessionRequiresExplicitStart() async {
        let controller = GazeSessionController()
        controller.calibrationTarget = CGPoint(x: 0.5, y: 0.5)
        for i in 0..<10 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
        }
        await controller.flushPipeline()
        XCTAssertNil(controller.captureSource)
        XCTAssertEqual(controller.trackingStage, .searchingFace, "Ticks without a simulated session must not touch UI state")
        let emptySummary = await controller.stopSession()
        XCTAssertEqual(emptySummary.totalFrames, 0)

        #if ASD_SIMULATION_ENABLED
        controller.startSimulatedSession()
        XCTAssertEqual(controller.captureSource, .simulated)
        XCTAssertTrue(controller.isSimulationMode)
        XCTAssertTrue(controller.isRecording)
        for i in 0..<10 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
        }
        let summary = await controller.stopSession()
        XCTAssertEqual(summary.totalFrames, 10)
        XCTAssertEqual(summary.captureSource, .simulated)
        #endif
    }

    #if ASD_SIMULATION_ENABLED
    @MainActor
    func testSessionControllerSimulationRun() async {
        let controller = GazeSessionController()
        controller.startSimulatedSession()
        XCTAssertTrue(controller.isRecording)

        // Simulate 30 frames at 60 Hz (0.5 seconds)
        for i in 0..<30 {
            let t = Double(i) / 60.0
            controller.tickSimulatedFrame(timeSec: t)
        }

        let summary = await controller.stopSession()
        XCTAssertFalse(controller.isRecording)
        XCTAssertEqual(summary.totalFrames, 30)
        XCTAssertEqual(summary.captureSource, .simulated)
        XCTAssertGreaterThan(summary.validRatio, 0.8)
        XCTAssertGreaterThanOrEqual(summary.socialPreferenceScore, -1.0)
        XCTAssertLessThanOrEqual(summary.socialPreferenceScore, 1.0)
        XCTAssertEqual(summary.interruptionCount, 0)
        XCTAssertEqual(summary.interruptedSec, 0)
    }

    // MARK: - PLC-1, PLC-2: no placeholder measurements

    @MainActor
    func testNoFaceHitsGiveNilDwellRatios() async throws {
        // GeoPref battery has no Eye_Region / Mouth_Region AOIs, so no face hit can occur.
        let controller = GazeSessionController()
        controller.startSimulatedSession(battery: .makeGeoPrefBattery())
        for i in 0..<60 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
        }
        let summary = await controller.stopSession()
        XCTAssertEqual(summary.totalFrames, 60)
        XCTAssertNil(summary.eyeDwellRatio, "No eye/mouth hit must yield nil, never 0.28")
        XCTAssertNil(summary.mouthDwellRatio, "No eye/mouth hit must yield nil, never 0.14")
        // GeoPref has a Social_Actor (orienting) AOI, so latency is measured, never the 480 default.
        let latency = try XCTUnwrap(summary.firstEyeLatencyMs)
        XCTAssertEqual(latency, 0.0, accuracy: 1e-6)
        XCTAssertNotEqual(latency, 480.0)
    }

    @MainActor
    func testFaceHitsGiveMeasuredDwellRatiosAndLatency() async throws {
        let controller = GazeSessionController()
        // Calibration target sits inside the face battery's Eye_Region.
        controller.calibrationTarget = CGPoint(x: 0.5, y: 0.38)
        controller.startSimulatedSession(battery: .makeFaceScanningBattery())
        for i in 0..<60 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
        }
        let summary = await controller.stopSession()
        XCTAssertEqual(summary.eyeDwellRatio, 1.0)
        XCTAssertEqual(summary.mouthDwellRatio, 0.0)
        XCTAssertEqual(try XCTUnwrap(summary.firstEyeLatencyMs), 0.0, accuracy: 1e-6)
    }

    @MainActor
    func testStopAndPersistCarriesCaptureSourceIntoHeader() async throws {
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: tempDir) }
        let vault = SessionVaultManager(vaultDirectory: tempDir, keyData: Data(repeating: 7, count: 32))

        let controller = GazeSessionController()
        controller.startSimulatedSession()
        for i in 0..<5 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
        }
        let result = try await controller.stopAndPersistBatchAsync(participantToken: "child-TEST0000", vault: vault)
        XCTAssertEqual(result.summary.captureSource, .simulated)
        XCTAssertTrue(result.vaultURL.lastPathComponent.hasSuffix(".sim.jsonl.enc"))
        let text = try vault.readSessionPlainText(from: result.vaultURL)
        XCTAssertTrue(text.contains("\"captureSource\":\"simulated\""))
    }
    #endif

    // MARK: - Vault

    func testVaultEncryptionAndDecryption() throws {
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: tempDir) }
        let vault = SessionVaultManager(vaultDirectory: tempDir)

        let header = SessionHeader.fixture(sessionId: "TEST1234")
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
        let report = GazeSessionReportSummary.fixture(
            sessionId: "TEST1234",
            totalFrames: 1,
            validFrames: 1,
            validRatio: 1.0,
            socialPreferenceScore: 0.55,
            socialDwellSec: 6.0,
            geometricDwellSec: 4.0
        )

        let fileURL = try vault.saveSession(header: header, frames: [frame], report: report)
        XCTAssertTrue(FileManager.default.fileExists(atPath: fileURL.path))
        XCTAssertTrue(fileURL.lastPathComponent.hasSuffix(".jsonl.enc"))
        XCTAssertFalse(fileURL.lastPathComponent.contains(".sim."), "Live sessions must not carry the simulated suffix")

        let decryptedText = try vault.readSessionPlainText(from: fileURL)
        XCTAssertTrue(decryptedText.contains("TEST1234"))
        XCTAssertTrue(decryptedText.contains("Social_Actor"))
        XCTAssertTrue(decryptedText.contains("socialPreferenceScore"))
        XCTAssertTrue(decryptedText.contains("\"captureSource\":\"truedepth\""))
    }

    // SIM-5 (vault), CNS-3
    func testVaultSimulatedSuffixAndProtection() throws {
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: tempDir) }
        let vault = SessionVaultManager(vaultDirectory: tempDir, keyData: Data(repeating: 1, count: 32))

        let header = SessionHeader.fixture(sessionId: "SIM00001", captureSource: .simulated)
        let report = GazeSessionReportSummary.fixture(sessionId: "SIM00001", captureSource: .simulated)
        let fileURL = try vault.saveSession(header: header, frames: [], report: report)

        XCTAssertTrue(fileURL.lastPathComponent.hasSuffix(".sim.jsonl.enc"), "Got \(fileURL.lastPathComponent)")
        XCTAssertTrue(fileURL.lastPathComponent.hasPrefix("session_SIM00001_"))

        // Files are written with complete file protection.
        XCTAssertTrue(SessionVaultManager.fileWritingOptions.contains(.completeFileProtection))
        XCTAssertTrue(SessionVaultManager.fileWritingOptions.contains(.atomic))
        #if os(iOS)
        let attributes = try FileManager.default.attributesOfItem(atPath: fileURL.path)
        XCTAssertEqual(attributes[.protectionKey] as? FileProtectionType, .complete)
        #endif

        // The vault directory is excluded from device backups.
        let values = try tempDir.resourceValues(forKeys: [.isExcludedFromBackupKey])
        XCTAssertEqual(values.isExcludedFromBackup, true)

        // Simulated files still decrypt for developer debugging.
        let text = try vault.readSessionPlainText(from: fileURL)
        XCTAssertTrue(text.contains("SIM00001"))
    }

    func testVaultKeyPersistsAcrossManagerInstances() throws {
        // Mirrors a relaunch: a manager created later must decrypt sessions written
        // by an earlier one. Skipped when the Keychain is unavailable in this
        // environment (both instances would then hold unrelated ephemeral keys).
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let first = SessionVaultManager(vaultDirectory: tempDir)
        try XCTSkipUnless(first.isKeyPersisted, "Keychain unavailable — key persistence not testable here")

        let header = SessionHeader.fixture(sessionId: "PERSIST1", stimulusDurationSec: 1.0)
        let frame = GazeFrame(
            gaze: GazeTelemetrySample(timestampMs: 0.0, xNorm: 0.5, yNorm: 0.5, confidence: 0.9),
            affect: .neutral,
            activeAOINames: [],
            aoiDwellMs: 16.7,
            qualityVerdict: .optimal
        )
        let report = GazeSessionReportSummary.fixture(
            sessionId: "PERSIST1",
            durationSec: 1.0,
            totalFrames: 1,
            validFrames: 1,
            validRatio: 1.0,
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
        let header = SessionHeader.fixture(sessionId: "AGESIG1", stimulusDurationSec: 5.0)
        let report = GazeSessionReportSummary.fixture(
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

    // MARK: - Export schemas (PLC-3 kit side)

    func testExportSchemas() throws {
        let report = GazeSessionReportSummary.fixture(
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
        XCTAssertEqual(rk["captureSource"] as? String, "truedepth")
        XCTAssertNil(rk["classification"])
        XCTAssertTrue(JSONSerialization.isValidJSONObject(rk))

        let ck = SessionVaultManager.makeCareKitJson(report: report)
        XCTAssertEqual(ck["identifier"] as? String, "OCKCarePlanActivity")
        XCTAssertEqual(ck["sessionId"] as? String, "EXP789")
        XCTAssertTrue(JSONSerialization.isValidJSONObject(ck))
    }

    func testResearchKitJsonOmitsAbsentMetrics() throws {
        let report = GazeSessionReportSummary.fixture(eyeDwellRatio: 0.3, mouthDwellRatio: nil, firstEyeLatencyMs: nil)
        let rk = SessionVaultManager.makeResearchKitJson(report: report)
        let biomarkers = try XCTUnwrap(rk["biomarkers"] as? [String: Any])
        XCTAssertEqual(biomarkers["eyeDwellRatio"] as? Double, 0.3)
        XCTAssertNil(biomarkers["mouthDwellRatio"], "Absent metric must be omitted, not defaulted")
        XCTAssertNil(biomarkers["firstEyeLatencyMs"], "Absent metric must be omitted, not defaulted")
        XCTAssertNoThrow(try JSONSerialization.data(withJSONObject: rk))
    }
}
