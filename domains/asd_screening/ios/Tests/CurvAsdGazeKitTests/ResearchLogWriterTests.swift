import AsdCore
import Foundation
import simd
import XCTest
@testable import CurvAsdGazeKit

/// WP-X.6 (RFL-2, acceptance 15 and 16, device half): the actor writes the research frame log
/// only when the app armed it (`NormCollectionGate.isActive`) for a live session, through the
/// Rust line functions, under a size cap, and leaves nothing behind otherwise.
final class ResearchLogWriterTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("research-log-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: directory)
    }

    private func configuration(
        capBytes: Int = ResearchLogWriter.defaultCapBytes,
        profileVersion: String? = nil,
        catalogVersion: String? = nil
    ) -> ResearchLogConfiguration {
        ResearchLogConfiguration(
            directory: directory,
            participantToken: "STUDY-ASD-TEST",
            bundleVersion: "2026.11.1",
            profileVersion: profileVersion,
            catalogVersion: catalogVersion,
            appVersion: "9.9",
            capBytes: capBytes
        )
    }

    private func files() -> [String] {
        ((try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? []).sorted()
    }

    /// One live session of `frames` frames at 60 Hz through the actor.
    private func runSession(
        _ actor: GazeProcessingActor,
        frames: Int,
        source: CaptureSource = .trueDepth
    ) async -> GazeSessionReportSummary {
        await actor.startSession(captureSource: source, stimulusId: "clip-a")
        for i in 0..<frames {
            _ = await actor.processFaceAnchor(
                snapshot: GazePipelineIntegrationTests.face(blink: i % 40 == 0 ? 0.9 : 0.05),
                elapsedSec: Double(i) / 60.0
            )
        }
        return await actor.finalizeSession().summary
    }

    private func lines(of url: URL) throws -> [String] {
        try String(contentsOf: url, encoding: .utf8)
            .split(separator: "\n", omittingEmptySubsequences: true)
            .map(String.init)
    }

    func testArmedLiveSessionWritesALogRustAccepts() async throws {
        let actor = GazeProcessingActor()
        await actor.armResearchLog(configuration())
        let summary = await runSession(actor, frames: 120)

        let url = ResearchLogWriter.fileURL(sessionId: summary.sessionId, in: directory)
        XCTAssertEqual(files(), [url.lastPathComponent], "only the finished log remains: no body, no marker")
        let all = try lines(of: url)
        XCTAssertEqual(all.count, 121, "header plus one line per frame")

        // The header is what the Rust header function emits for it, and names the session.
        let header = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(all[0].utf8)) as? [String: Any])
        XCTAssertEqual(header["format"] as? String, "asd-raw-tracking/1")
        XCTAssertEqual(header["sessionId"] as? String, summary.sessionId)
        XCTAssertEqual(header["participantToken"] as? String, "STUDY-ASD-TEST")
        XCTAssertEqual(header["bundleVersion"] as? String, "2026.11.1")
        let clips = try XCTUnwrap(header["clips"] as? [[String: Any]])
        XCTAssertEqual(clips.first?["clipId"] as? String, "clip-a")
        XCTAssertEqual(clips.first?["order"] as? Int, 0)
        XCTAssertNotNil(clips.first?["aois"])
        XCTAssertEqual(try gazeResearchLogHeaderLine(headerJson: all[0]), all[0], "header is a Rust fixed point")

        // Every sample line is canonical Rust output (re-encoding changes nothing), in strictly
        // increasing time, with affect on the recorded frames.
        var last = -Double.infinity
        var withAffect = 0
        for line in all.dropFirst() {
            XCTAssertEqual(try gazeResearchLogSampleLine(sampleJson: line), line)
            let object = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any])
            let t = try XCTUnwrap(object["t_sec"] as? Double)
            XCTAssertGreaterThan(t, last)
            last = t
            if object["affect"] != nil { withAffect += 1 }
        }
        XCTAssertEqual(withAffect, 120)

        // Optional: `ASD_CLI` points at a built `asd-cli`; the log must pass `gaze replay --check`.
        if let cli = ProcessInfo.processInfo.environment["ASD_CLI"] {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: cli)
            process.arguments = ["gaze", "replay", "--check", url.path]
            try process.run()
            process.waitUntilExit()
            XCTAssertEqual(process.terminationStatus, 0, "asd-cli gaze replay --check accepts the log")
        }
    }

    func testHeaderCarriesProfileAndCatalogVersionsWhenKnown() async throws {
        let actor = GazeProcessingActor()
        await actor.armResearchLog(configuration(profileVersion: "2026.11.1", catalogVersion: "2026.11.1"))
        let summary = await runSession(actor, frames: 10)
        let url = ResearchLogWriter.fileURL(sessionId: summary.sessionId, in: directory)
        let header = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(try lines(of: url)[0].utf8)) as? [String: Any])
        XCTAssertEqual(header["profileVersion"] as? String, "2026.11.1")
        let clips = try XCTUnwrap(header["clips"] as? [[String: Any]])
        XCTAssertEqual(clips.first?["catalogVersion"] as? String, "2026.11.1")
    }

    func testHeaderOmitsVersionsWhenUnknown() async throws {
        let actor = GazeProcessingActor()
        await actor.armResearchLog(configuration())
        let summary = await runSession(actor, frames: 10)
        let url = ResearchLogWriter.fileURL(sessionId: summary.sessionId, in: directory)
        let header = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(try lines(of: url)[0].utf8)) as? [String: Any])
        XCTAssertNil(header["profileVersion"])
        XCTAssertNil((header["clips"] as? [[String: Any]])?.first?["catalogVersion"])
    }

    func testNoFileWhenTheGateIsOff() async {
        let actor = GazeProcessingActor()
        _ = await runSession(actor, frames: 30)
        XCTAssertEqual(files(), [])
    }

    func testNoFileWhenCaptureIsSimulated() async {
        let actor = GazeProcessingActor()
        await actor.armResearchLog(configuration())
        _ = await runSession(actor, frames: 30, source: .simulated)
        XCTAssertEqual(files(), [], "an armed but simulated session writes nothing, not even a body file")
    }

    func testArmingAppliesToOneSessionOnly() async {
        let actor = GazeProcessingActor()
        await actor.armResearchLog(configuration())
        _ = await runSession(actor, frames: 10)
        let afterFirst = files()
        XCTAssertEqual(afterFirst.count, 1)
        let second = await runSession(actor, frames: 10)
        XCTAssertEqual(files(), afterFirst, "the second session was not armed: no new log for \(second.sessionId)")
    }

    func testCapDropsTheLogAndLeavesAMarker() async {
        let actor = GazeProcessingActor()
        await actor.armResearchLog(configuration(capBytes: 4_000))
        let summary = await runSession(actor, frames: 120)
        XCTAssertEqual(files(), [ResearchLogWriter.droppedMarkerName(sessionId: summary.sessionId)])
        XCTAssertFalse(FileManager.default.fileExists(
            atPath: ResearchLogWriter.fileURL(sessionId: summary.sessionId, in: directory).path
        ))
    }

    func testWriterAssemblesHeaderFirstAndNeverExceedsCap() throws {
        let writer = try XCTUnwrap(ResearchLogWriter(directory: directory, capBytes: 100))
        writer.append(line: String(repeating: "a", count: 40))
        writer.append(line: String(repeating: "b", count: 40))
        XCTAssertTrue(writer.isActive)
        // 82 bytes of body plus a 30 byte header is over 100.
        XCTAssertEqual(writer.finish(headerLine: String(repeating: "h", count: 29), sessionId: "S1"), .dropped)
        XCTAssertEqual(files(), ["research_S1.dropped"])

        let ok = try XCTUnwrap(ResearchLogWriter(directory: directory, capBytes: 1000))
        ok.append(line: "one")
        ok.append(line: "two")
        guard case .written(let url) = ok.finish(headerLine: "head", sessionId: "S2") else {
            return XCTFail("a log under the cap is written")
        }
        XCTAssertEqual(try String(contentsOf: url, encoding: .utf8), "head\none\ntwo\n")

        let empty = try XCTUnwrap(ResearchLogWriter(directory: directory))
        XCTAssertEqual(empty.finish(headerLine: "head", sessionId: "S3"), .empty)
        XCTAssertFalse(files().contains("research_S3.jsonl"))
        XCTAssertFalse(files().contains("research_active.body"))
    }

    func testLogIsProtectedAndExcludedFromBackup() throws {
        let writer = try XCTUnwrap(ResearchLogWriter(directory: directory))
        writer.append(line: "x")
        guard case .written(let url) = writer.finish(headerLine: "h", sessionId: "S4") else {
            return XCTFail("written")
        }
        let values = try url.resourceValues(forKeys: [.isExcludedFromBackupKey])
        XCTAssertEqual(values.isExcludedFromBackup, true)
    }
}
