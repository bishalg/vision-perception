#if ASD_BENCH_CAPTURE
@testable import AsdGazeCollector
import AsdCore
import CurvAsdGazeKit
import simd
import XCTest

/// WP-G.8 / BEN-2 / REP-1: the bench writer hands ARKit frames to the Rust encoder
/// (`gazeRawLogDocument`), so `asd-raw-tracking/1` is defined once in `asd-core`. The committed
/// sample from WP-G.1 is the oracle, so a renamed or dropped key, or a conversion slip on the
/// Swift side, fails here before a recording is wasted on a device.
final class BenchLogWriterTests: XCTestCase {
    private static var fixtureURL: URL {
        // Tests/ -> ios_asd_gaze_app/ -> apps/ -> repository root
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("rust/crates/asd-core/tests/fixtures/raw_tracking_sample.jsonl")
    }

    private func fixtureLines() throws -> [[String: Any]] {
        let text = try String(contentsOf: Self.fixtureURL, encoding: .utf8)
        return try text.split(separator: "\n").map { line in
            try XCTUnwrap(JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any])
        }
    }

    private func object(_ line: String) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any])
    }

    /// JSON equality with numbers compared to 1e-12 (Foundation's decimal parsing differs from
    /// serde in the last digit of values such as 0.07).
    private func assertJSONEqual(_ a: Any, _ b: Any, path: String = "$", file: StaticString = #filePath, line: UInt = #line) {
        switch (a, b) {
        case let (a as [String: Any], b as [String: Any]):
            XCTAssertEqual(Set(a.keys), Set(b.keys), path, file: file, line: line)
            for key in a.keys where b[key] != nil {
                assertJSONEqual(a[key]!, b[key]!, path: "\(path).\(key)", file: file, line: line)
            }
        case let (a as [Any], b as [Any]):
            XCTAssertEqual(a.count, b.count, path, file: file, line: line)
            for (i, (x, y)) in zip(a, b).enumerated() {
                assertJSONEqual(x, y, path: "\(path)[\(i)]", file: file, line: line)
            }
        case let (a as NSNumber, b as NSNumber):
            XCTAssertEqual(a.doubleValue, b.doubleValue, accuracy: 1e-12, path, file: file, line: line)
        case let (a as String, b as String):
            XCTAssertEqual(a, b, path, file: file, line: line)
        default:
            XCTFail("type mismatch at \(path): \(a) vs \(b)", file: file, line: line)
        }
    }

    private func translation(_ x: Float, _ y: Float, _ z: Float) -> simd_float4x4 {
        var m = matrix_identity_float4x4
        m.columns.3 = simd_float4(x, y, z, 1)
        return m
    }

    /// The tracked sample of the committed fixture (line 2), rebuilt from ARKit-typed values.
    private var trackedSample: BenchRawSample {
        BenchRawSample(
            snapshot: FaceAnchorSnapshot(
                isTracked: true,
                transform: translation(0.004, -0.11, -0.45),
                lookAtPoint: simd_float3(0.002, 0.04, 0.25),
                leftEyeTransform: translation(0.031, 0.025, 0.02),
                rightEyeTransform: translation(-0.031, 0.025, 0.02),
                blendShapes: ["eyeBlinkLeft": 0.05, "eyeBlinkRight": 0.07, "jawOpen": 0.4],
                ambientIntensity: 1000,
                ambientColorTemperature: 5500
            ),
            timestamp: 100.0,
            orientation: .portrait
        )
    }

    private var untrackedSample: BenchRawSample {
        BenchRawSample(
            snapshot: FaceAnchorSnapshot(
                isTracked: false,
                transform: matrix_identity_float4x4,
                lookAtPoint: .zero,
                ambientIntensity: 1000,
                ambientColorTemperature: 5500
            ),
            timestamp: 100.05,
            orientation: .portrait
        )
    }

    /// The header of the committed fixture: Rust resolves the geometry from the model.
    private func fixtureHeader() -> BenchLogHeader {
        BenchLogHeader(modelIdentifier: "iPhone17,3", orientation: .portrait, appVersion: "1.0 (1)", condition: .baseline)
    }

    private var fixtureTarget: BenchTargetRecord {
        BenchTargetRecord(kind: .calibration, xNorm: 0.5, yNorm: 0.5, startSec: 100.0, endSec: 101.6, label: "cal-1")
    }

    private func temporaryDirectory() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("bench-\(UUID().uuidString)")
    }

    // MARK: - Whole log (encoded by Rust, REP-1)

    /// Frames handed over exactly as the AR tap delivers them become the committed fixture,
    /// line for line: the writer adds no format of its own.
    func testWriterReproducesTheCommittedFixture() throws {
        let dir = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let writer = BenchLogWriter(directory: dir, baseName: "unit", header: fixtureHeader())
        writer.append(trackedSample)
        let second = BenchRawSample(snapshot: trackedSample.snapshot, timestamp: 100.016667, orientation: .portrait)
        writer.append(second)
        // A repeated frame time would break REP-1 (strictly increasing): dropped, counted.
        writer.append(second)
        writer.append(untrackedSample)
        writer.recordTarget(fixtureTarget)
        let result = try writer.finish()

        XCTAssertEqual(result.sampleCount, 3)
        XCTAssertEqual(result.droppedSampleCount, 1)
        let text = try String(contentsOf: result.rawLogURL, encoding: .utf8)
        XCTAssertTrue(text.hasSuffix("\n"))
        let actual = try text.split(separator: "\n").map { try object(String($0)) }
        let expected = try fixtureLines()
        XCTAssertEqual(actual.count, expected.count)
        for (a, e) in zip(actual, expected) {
            XCTAssertEqual(Set(a.keys), Set(e.keys), "key names differ from the G.1 fixture")
            assertJSONEqual(a, e)
        }
        XCTAssertNil(actual[3]["face_transform"], "zeros would read as a perfect frontal pose")

        let targetLog = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: result.targetLogURL)) as? [Any]
        )
        assertJSONEqual(targetLog, try XCTUnwrap(expected[0]["targets"]))
    }

    func testHeaderConditionIsTheRustNameAndOptionalFieldsAreOmitted() throws {
        let dir = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        var header = fixtureHeader()
        header.condition = .lowLight
        var result = try BenchLogWriter(directory: dir, baseName: "a", header: header).finish()
        var first = try XCTUnwrap(try String(contentsOf: result.rawLogURL, encoding: .utf8).split(separator: "\n").first)
        XCTAssertEqual(try object(String(first))["condition"] as? String, "lowLight")

        header.condition = nil
        header.appVersion = nil
        result = try BenchLogWriter(directory: dir, baseName: "b", header: header).finish()
        first = try XCTUnwrap(try String(contentsOf: result.rawLogURL, encoding: .utf8).split(separator: "\n").first)
        let v = try object(String(first))
        XCTAssertNil(v["condition"])
        XCTAssertNil(v["appVersion"])
        XCTAssertEqual(result.sampleCount, 0)
    }

    /// The bench picker offers the BEN-4 matrix from Rust, and its names are file-name safe.
    @MainActor
    func testBenchConditionsComeFromRustAndAreFileNameSafe() {
        let names = gazeBenchConditions().map { gazeBenchConditionName(condition: $0) }
        XCTAssertEqual(names, [
            "baseline", "distance", "head", "motion", "glasses",
            "lowLight", "sideLight", "occlusion", "blinks"
        ])
        for name in names {
            XCTAssertEqual(BenchLogWriter.sanitisedName(name), name)
        }
        XCTAssertEqual(BenchCaptureModel.conditions, gazeBenchConditions())
    }

    // MARK: - File

    func testFileNamesAreSanitisedProtectedAndBackupExcluded() throws {
        let dir = temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        XCTAssertEqual(BenchLogWriter.sanitisedName("low light/../50 lux"), "low-light----50-lux")
        let writer = BenchLogWriter(
            directory: dir, baseName: "x/../y",
            header: BenchLogHeader(modelIdentifier: "d", orientation: .portrait, appVersion: nil, condition: nil)
        )
        let result = try writer.finish()
        XCTAssertEqual(result.rawLogURL.lastPathComponent, "x----y.jsonl")
        XCTAssertEqual(result.rawLogURL.deletingLastPathComponent().standardizedFileURL, dir.standardizedFileURL)
        let values = try dir.resourceValues(forKeys: [.isExcludedFromBackupKey])
        XCTAssertEqual(values.isExcludedFromBackup, true, "BEN-8: bench logs are biometric data")
    }

    // MARK: - Targets and grid (layouts come from Rust, never computed here)

    func testRunConfigReadsLayoutsFromTheRustConfig() throws {
        let config = try BenchRunConfig.load(modelIdentifier: "iPhone17,3", orientation: .portrait)
        XCTAssertEqual(config.modelIdentifier, "iPhone17,3")
        XCTAssertEqual(config.geometrySource, "table")
        XCTAssertEqual(config.layout5.count, 5)
        XCTAssertEqual(config.layout9.count, 9)
        XCTAssertEqual(config.grid13.count, 13)
        // VAL-5: 3x3 at 0.1/0.5/0.9 plus 4 at 0.3/0.7.
        var expected = Set<String>()
        for x in [0.1, 0.5, 0.9] { for y in [0.1, 0.5, 0.9] { expected.insert("\(x),\(y)") } }
        for x in [0.3, 0.7] { for y in [0.3, 0.7] { expected.insert("\(x),\(y)") } }
        XCTAssertEqual(Set(config.grid13.map { "\($0[0]),\($0[1])" }), expected)
        XCTAssertGreaterThan(config.settleSec + config.collectSec, 0)
    }

    func testSequencesUseCalibrationKindForFiveAndNineAndValidationForTheGrid() throws {
        let config = try BenchRunConfig.load(modelIdentifier: "iPhone17,3", orientation: .portrait)
        XCTAssertEqual(BenchSequence.fivePoint.plan(config).count, 5)
        XCTAssertEqual(BenchSequence.ninePoint.plan(config).count, 9)
        let grid = BenchSequence.grid13.plan(config)
        XCTAssertEqual(grid.count, 13)
        XCTAssertTrue(BenchSequence.fivePoint.plan(config).allSatisfy { $0.kind == .calibration })
        XCTAssertTrue(BenchSequence.ninePoint.plan(config).allSatisfy { $0.kind == .calibration })
        XCTAssertTrue(grid.allSatisfy { $0.kind == .validation })
        XCTAssertEqual(grid.map(\.label), (1...13).map { "grid13-\($0)" })
    }

    func testFlagNameConstantIsTheReleaseSymbolNeedle() {
        // scripts/check_release_symbols.sh greps the Release binary for this string.
        XCTAssertEqual(BenchCaptureFlag.name, "ASD_BENCH_CAPTURE")
    }
}
#endif
