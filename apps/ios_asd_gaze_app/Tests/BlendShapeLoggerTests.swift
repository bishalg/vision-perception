#if ASD_ML_DATA_EXPORT
@testable import AsdGazeCollector
import CurvAsdGazeKit
import Foundation
import XCTest

/// CNS-2: the blend-shape CSV logger exists only under `ASD_ML_DATA_EXPORT`, and
/// even then writes only while recording for a participant who opted in.
final class BlendShapeLoggerTests: XCTestCase {
    private var directory: URL!
    private var logger: BlendShapeCSVLogger?

    override func setUp() {
        super.setUp()
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("BlendShapeML_\(UUID().uuidString)", isDirectory: true)
    }

    override func tearDown() {
        logger?.stopListening()
        logger = nil
        try? FileManager.default.removeItem(at: directory)
        super.tearDown()
    }

    func testNoWriteWhenNormCollectionNotOptedIn() {
        let logger = makeLogger(optedIn: false)
        post(recording: true)
        logger.stopListening()
        XCTAssertEqual(csvFiles(), [], "No CSV may be written without norm-collection opt-in")
    }

    func testNoWriteWhenNotRecording() {
        let logger = makeLogger(optedIn: true)
        post(recording: false)
        logger.stopListening()
        XCTAssertEqual(csvFiles(), [])
    }

    func testWritesProtectedFileInBackupExcludedDirectoryWhenOptedInAndRecording() throws {
        let logger = makeLogger(optedIn: true)
        post(recording: true)
        logger.stopListening()

        let file = try XCTUnwrap(csvFiles().first)
        let text = try String(contentsOf: file, encoding: .utf8)
        XCTAssertTrue(text.hasPrefix(logger.headerLine))
        XCTAssertEqual(text.split(separator: "\n").count, 2, "Header plus one data row")

        XCTAssertTrue(BlendShapeCSVLogger.fileWriteOptions.contains(.completeFileProtection))
        #if !targetEnvironment(simulator)
        // The simulator neither records nor reports per-file protection classes.
        let attrs = try FileManager.default.attributesOfItem(atPath: file.path)
        XCTAssertEqual(attrs[.protectionKey] as? FileProtectionType, .complete)
        #endif
        let values = try directory.resourceValues(forKeys: [.isExcludedFromBackupKey])
        XCTAssertEqual(values.isExcludedFromBackup, true)
    }

    // MARK: - Helpers

    private func makeLogger(optedIn: Bool) -> BlendShapeCSVLogger {
        let logger = BlendShapeCSVLogger(directory: directory, isNormCollectionOptedIn: { optedIn })
        logger.startListening()
        self.logger = logger
        return logger
    }

    private func post(recording: Bool) {
        NotificationCenter.default.post(
            name: GazeSessionController.didUpdateFaceBlendShapesNotification,
            object: nil,
            userInfo: [
                GazeSessionController.isRecordingUserInfoKey: recording,
                GazeSessionController.blendShapesUserInfoKey: ["jawOpen": 0.5],
                GazeSessionController.sessionIdUserInfoKey: "TEST-SESSION"
            ]
        )
    }

    private func csvFiles() -> [URL] {
        ((try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? [])
            .filter { $0.pathExtension == "csv" }
    }
}
#endif
