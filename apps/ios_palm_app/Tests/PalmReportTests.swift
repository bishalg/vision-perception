import XCTest
import CurvPalmOverlay
@testable import PalmScanner

final class PalmReportTests: XCTestCase {

    private func reliefMap(_ entries: [PalmReportTestFactory.Entry]) -> MountReliefMap {
        PalmReportTestFactory.reliefMap(entries)
    }

    private let uprightLandmarks: [CGPoint] = [
        CGPoint(x: 0.50, y: 0.85), // 0 wrist
        CGPoint(x: 0.40, y: 0.72), CGPoint(x: 0.32, y: 0.62),
        CGPoint(x: 0.28, y: 0.54), CGPoint(x: 0.24, y: 0.48),
        CGPoint(x: 0.38, y: 0.50), CGPoint(x: 0.36, y: 0.38),
        CGPoint(x: 0.35, y: 0.28), CGPoint(x: 0.34, y: 0.20),
        CGPoint(x: 0.50, y: 0.48), CGPoint(x: 0.50, y: 0.34),
        CGPoint(x: 0.50, y: 0.24), CGPoint(x: 0.50, y: 0.15),
        CGPoint(x: 0.62, y: 0.51), CGPoint(x: 0.63, y: 0.37),
        CGPoint(x: 0.64, y: 0.27), CGPoint(x: 0.65, y: 0.19),
        CGPoint(x: 0.72, y: 0.55), CGPoint(x: 0.74, y: 0.44),
        CGPoint(x: 0.75, y: 0.36), CGPoint(x: 0.76, y: 0.29),
    ]

    // MARK: - Ranking

    func testMountsRankedStrongestMeasuredFirst() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([
                .init(mount: .jupiter, mm: 1.0, rel: 0.4, prom: .average, conf: 0.9, views: 10),
                .init(mount: .venus, mm: 2.8, rel: 0.9, prom: .elevated, conf: 0.9, views: 12),
                .init(mount: .luna, mm: 0.2, rel: 0.1, prom: .deficient, conf: 0.9, views: 8),
            ]),
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )

        let ranked = report.mountReports
        XCTAssertEqual(ranked.first?.id, "venus", "highest measured mm must rank first")
        XCTAssertEqual(ranked[1].id, "jupiter")
        XCTAssertEqual(ranked.filter(\.isMeasured).map(\.id), ["venus", "jupiter", "luna"],
                       "measured mounts rank strongest-first")
        XCTAssertFalse(ranked.last!.isMeasured,
                       "the four unmeasured mounts trail after every measured one")
    }

    func testUnmeasuredMountsSortLastButStayPresent() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([
                .init(mount: .saturn, mm: 1.2, rel: 0.4, prom: .average, conf: 0.85, views: 9),
            ]),
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )

        let ranked = report.mountReports
        XCTAssertEqual(ranked.count, 7, "all seven mounts stay in the report")
        XCTAssertEqual(ranked.first?.id, "saturn", "the only measured mount ranks first")
        XCTAssertFalse(ranked.last!.isMeasured, "unmeasured mounts are placed after measured ones")
    }

    // MARK: - Dominance & synthesis

    func testDominantMountsPickTopTwoReliable() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([
                .init(mount: .venus, mm: 2.8, rel: 0.9, prom: .elevated, conf: 0.9, views: 12),
                .init(mount: .jupiter, mm: 2.3, rel: 0.8, prom: .elevated, conf: 0.88, views: 11),
                .init(mount: .apollo, mm: 2.6, rel: 0.85, prom: .elevated, conf: 0.3, views: 4),
            ]),
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )

        let ids = report.dominantMounts.map(\.id)
        XCTAssertEqual(ids, ["venus", "jupiter"],
                       "low-confidence apollo must be excluded from dominance despite high mm")
        XCTAssertTrue(report.synthesis.contains("Venus"))
        XCTAssertTrue(report.synthesis.contains("Jupiter"))
        XCTAssertFalse(report.synthesis.contains("anchored by two strong regions — the Venus and the Venus"))
    }

    func testSynthesisFallsBackWhenNothingMeasured() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: nil,
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )
        XCTAssertTrue(report.synthesis.contains("No mount relief was captured"),
                      "empty analysis must produce the guidance fallback, not an empty string")
        XCTAssertNotNil(report.dataQualityNote,
                        "an empty scan should tell the reader that mounts were not measured")
    }

    func testEvenProfileSynthesisWhenMountsAreClose() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([
                .init(mount: .venus, mm: 1.45, rel: 0.5, prom: .average, conf: 0.9, views: 10),
                .init(mount: .jupiter, mm: 1.5, rel: 0.5, prom: .average, conf: 0.9, views: 10),
            ]),
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )
        // Both dominant (2 reliable mounts) — synthesis names both, not the "even" fallback.
        XCTAssertTrue(report.synthesis.contains("anchored by two strong regions"))
    }

    // MARK: - Robustness: missing / partial data

    func testUnmeasuredMountCarriesPlaceholderReading() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([
                .init(mount: .venus, mm: 2.5, rel: 0.8, prom: .elevated, conf: 0.9, views: 12),
            ]),
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )
        let mercury = report.mountReports.first { $0.id == "mercury" }
        XCTAssertNotNil(mercury)
        XCTAssertFalse(mercury!.isMeasured)
        XCTAssertNil(mercury!.development)
        XCTAssertTrue(mercury!.reading.contains("not measured clearly"),
                      "unmeasured mounts explain themselves instead of showing a fabricated reading")
    }

    func testPadDepthFallbackUsedWhenReliefMapMissing() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: nil,
            padDepths: ["venus": 2.7, "jupiter": 0.1],
            landmarks: uprightLandmarks,
            handSide: .right
        )
        let venus = report.mountReports.first { $0.id == "venus" }
        XCTAssertEqual(venus?.heightMm, 2.7)
        XCTAssertEqual(venus?.development, .prominent)
        let jupiter = report.mountReports.first { $0.id == "jupiter" }
        XCTAssertEqual(jupiter?.development, .deficient)
        XCTAssertTrue(report.dominantMounts.map(\.id).contains("venus"))
    }

    func testDataQualityNoteFlagsUnmeasuredAndLowConfidence() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([
                .init(mount: .venus, mm: 2.5, rel: 0.8, prom: .elevated, conf: 0.4, views: 2),
                .init(mount: .jupiter, mm: 2.0, rel: 0.7, prom: .elevated, conf: 0.9, views: 12),
            ]),
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )
        let note = report.dataQualityNote
        XCTAssertNotNil(note)
        XCTAssertTrue(note!.contains("5 of 7 mounts were not clearly measured"), "note=\(note ?? "nil")")
        XCTAssertTrue(note!.contains("Lower confidence on Mount of Venus"), "note=\(note ?? "nil")")
    }

    func testCleanFullScanHasNoQualityNote() {
        let entries: [PalmReportTestFactory.Entry] = [
            .init(mount: .venus, mm: 2.5, rel: 0.8, prom: .elevated, conf: 0.9, views: 12),
            .init(mount: .jupiter, mm: 2.3, rel: 0.76, prom: .elevated, conf: 0.9, views: 12),
            .init(mount: .saturn, mm: 1.5, rel: 0.5, prom: .average, conf: 0.85, views: 12),
            .init(mount: .apollo, mm: 1.6, rel: 0.53, prom: .average, conf: 0.86, views: 12),
            .init(mount: .mercury, mm: 1.2, rel: 0.4, prom: .average, conf: 0.8, views: 11),
            .init(mount: .luna, mm: 2.0, rel: 0.66, prom: .elevated, conf: 0.78, views: 10),
            .init(mount: .mars, mm: -1.0, rel: 0.18, prom: .deficient, conf: 0.72, views: 9),
        ]
        let report = PalmReportGenerator.generate(
            mountReliefMap: reliefMap(entries),
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )
        XCTAssertNil(report.dataQualityNote, "a complete, confident scan needs no caveat")
        XCTAssertEqual(report.dominantMounts.map(\.id), ["venus", "jupiter"])
    }

    // MARK: - Biometrics grounding

    func testShapeAndRatioReadingsPopulatedFromLandmarks() {
        let report = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([
                .init(mount: .venus, mm: 2.5, rel: 0.8, prom: .elevated, conf: 0.9, views: 12),
            ]),
            padDepths: [:],
            landmarks: uprightLandmarks,
            handSide: .right
        )
        XCTAssertNotNil(report.handShapeLabel)
        XCTAssertNotNil(report.handShapeReading)
        XCTAssertNotNil(report.ratio2D4D)
        XCTAssertNotNil(report.ratio2D4DReading)
        XCTAssertGreaterThan(report.palmBreadthMm ?? 0, 0)
        XCTAssertGreaterThan(report.palmLengthMm ?? 0, 0)
        // Landmark fixture has index ~ pinky knuckle span vs length: expect a known tier text.
        XCTAssertTrue(["earth hand", "water hand", "even blend"].contains { report.handShapeReading!.contains($0) })
    }

    func testHandContextReflectsHandSide() {
        let left = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([.init(mount: .venus, mm: 2.0, rel: 0.7, prom: .elevated, conf: 0.9, views: 10)]),
            padDepths: [:], landmarks: uprightLandmarks, handSide: .left
        )
        let right = PalmReportGenerator.generate(
            mountReliefMap: reliefMap([.init(mount: .venus, mm: 2.0, rel: 0.7, prom: .elevated, conf: 0.9, views: 10)]),
            padDepths: [:], landmarks: uprightLandmarks, handSide: .right
        )
        XCTAssertTrue(left.handContext.contains("Left"))
        XCTAssertTrue(right.handContext.contains("Right"))
    }
}

/// Small factory keeping the test cases terse.
private enum PalmReportTestFactory {
    struct Entry {
        let mount: PalmMount
        let mm: Float
        let rel: Float
        let prom: MountProminence
        let conf: Float
        let views: Int
    }

    static func reliefMap(_ entries: [Entry]) -> MountReliefMap {
        MountReliefMap(
            mounts: entries.map {
                MountReliefEntry(
                    mount: $0.mount.rawValue,
                    heightMm: $0.mm,
                    heightRelative: $0.rel,
                    prominence: $0.prom,
                    confidence: $0.conf,
                    viewCount: $0.views
                )
            },
            venusElevated: entries.contains { $0.mount == .venus && $0.prom == .elevated },
            source: "test"
        )
    }
}
