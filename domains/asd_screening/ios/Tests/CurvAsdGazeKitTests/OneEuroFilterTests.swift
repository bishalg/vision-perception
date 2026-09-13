import XCTest
import CoreGraphics
@testable import CurvAsdGazeKit

final class OneEuroFilterTests: XCTestCase {
    func testStationarySignalJitterReduction() {
        let filter = OneEuroFilter(minCutoff: 1.0, beta: 0.01, dCutoff: 1.0)
        let trueValue = 0.50
        var filteredValues: [Double] = []

        // Simulate 60 frames with high-frequency noise (jitter ±0.03)
        for i in 0..<60 {
            let t = Double(i) / 60.0
            let noise = 0.03 * sin(Double(i) * 5.0)
            let sample = trueValue + noise
            let filtered = filter.filter(value: sample, timestamp: t)
            filteredValues.append(filtered)
        }

        // Compare variance of raw noise vs filtered in the second half
        let rawNoiseMax = 0.03
        let lateFiltered = Array(filteredValues[30..<60])
        let maxError = lateFiltered.map { abs($0 - trueValue) }.max() ?? 1.0

        // Filter should significantly reduce jitter (error should be much smaller than raw noise)
        XCTAssertLessThan(maxError, rawNoiseMax * 0.45)
    }

    func testFastSaccadeTransitionResponsiveness() {
        let filter = OneEuroFilter(minCutoff: 1.0, beta: 0.02, dCutoff: 1.0)

        // Settle at 0.20
        for i in 0..<30 {
            _ = filter.filter(value: 0.20, timestamp: Double(i) / 60.0)
        }

        // Rapid saccade to 0.80 at frame 31
        var val = 0.20
        for i in 30..<45 {
            val = filter.filter(value: 0.80, timestamp: Double(i) / 60.0)
        }

        // After 15 frames (250ms), cursor should have tracked substantially towards 0.80
        XCTAssertGreaterThan(val, 0.65)
    }

    func test2DFilterPointSmoothing() {
        let filter2D = OneEuroFilter2D(minCutoff: 1.2, beta: 0.015, dCutoff: 1.0)
        let pt = CGPoint(x: 0.35, y: 0.65)
        let filtered = filter2D.filter(point: pt, timestamp: 1.0)

        XCTAssertEqual(filtered.x, pt.x, accuracy: 0.001)
        XCTAssertEqual(filtered.y, pt.y, accuracy: 0.001)
    }

    func testFilterReset() {
        let filter = OneEuroFilter()
        _ = filter.filter(value: 100.0, timestamp: 1.0)
        filter.reset()
        // After reset, first sample is taken immediately as base
        let val = filter.filter(value: 0.0, timestamp: 2.0)
        XCTAssertEqual(val, 0.0, accuracy: 1e-6)
    }

    func testConcurrentFilterAndResetThreadSafety() {
        let filter2D = OneEuroFilter2D(minCutoff: 1.2, beta: 0.015, dCutoff: 1.0)
        let queue = DispatchQueue(label: "org.curv.test.concurrent", attributes: .concurrent)
        let group = DispatchGroup()

        // 100 concurrent read/write and reset calls across threads
        for i in 0..<100 {
            group.enter()
            queue.async {
                let t = Double(i) * 0.016
                let pt = CGPoint(x: 0.5 + Double(i % 10) * 0.01, y: 0.5)
                _ = filter2D.filter(point: pt, timestamp: t)
                if i % 25 == 0 {
                    filter2D.reset()
                }
                group.leave()
            }
        }

        let result = group.wait(timeout: .now() + 2.0)
        XCTAssertEqual(result, .success, "Concurrent filtering and resetting should complete without deadlock or crash")
    }
}
