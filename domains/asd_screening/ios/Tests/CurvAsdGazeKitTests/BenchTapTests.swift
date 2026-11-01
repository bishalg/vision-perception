#if ASD_BENCH_CAPTURE
import os
import simd
import XCTest
@testable import CurvAsdGazeKit

/// WP-G.8 / BEN-2: the Debug-only bench tap on `ARSessionBridge` sees every frame with the
/// interface orientation last given to `updateOrientation`, and stops when removed.
final class BenchTapTests: XCTestCase {
    private final class Received: @unchecked Sendable {
        private let lock = OSAllocatedUnfairLock(initialState: [BenchRawSample]())
        func append(_ sample: BenchRawSample) { lock.withLock { $0.append(sample) } }
        var samples: [BenchRawSample] { lock.withLock { $0 } }
    }

    private static let snapshot = FaceAnchorSnapshot(
        isTracked: true,
        transform: matrix_identity_float4x4,
        lookAtPoint: simd_float3(0, 0, 1)
    )

    func testTapReceivesTimestampAndLastOrientation() {
        let bridge = ARSessionBridge(processingActor: GazeProcessingActor())
        let received = Received()
        bridge.setBenchTap { received.append($0) }

        bridge.benchEmit(Self.snapshot, timestamp: 10.5)
        bridge.updateOrientation(.landscapeLeft)
        bridge.benchEmit(Self.snapshot, timestamp: 10.6)
        bridge.updateOrientation(.landscapeRight)
        bridge.updateOrientation(.portrait)
        bridge.benchEmit(Self.snapshot, timestamp: 10.7)

        let samples = received.samples
        XCTAssertEqual(samples.map(\.timestamp), [10.5, 10.6, 10.7])
        XCTAssertEqual(samples.map(\.orientation), [.portrait, .landscapeLeft, .portrait])
        XCTAssertEqual(samples.first?.snapshot.isTracked, true)
    }

    func testRemovingTheTapStopsEmission() {
        let bridge = ARSessionBridge(processingActor: GazeProcessingActor())
        let received = Received()
        bridge.setBenchTap { received.append($0) }
        bridge.benchEmit(Self.snapshot, timestamp: 1)

        bridge.setBenchTap(nil)
        bridge.benchEmit(Self.snapshot, timestamp: 2)
        bridge.benchEmit(Self.snapshot, timestamp: 3)

        XCTAssertEqual(received.samples.map(\.timestamp), [1])
    }

    func testNoTapNoEmission() {
        let bridge = ARSessionBridge(processingActor: GazeProcessingActor())
        bridge.benchEmit(Self.snapshot, timestamp: 1) // must not crash
    }
}
#endif
