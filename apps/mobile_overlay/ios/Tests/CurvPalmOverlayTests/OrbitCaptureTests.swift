import XCTest
@testable import CurvCameraOverlay
@testable import CurvPalmOverlay

final class OrbitCaptureTests: XCTestCase {
    func testRigidHandCheckerRejectsCurledFingers() {
        var ref = PalmRegionGeometry.scaffoldLandmarks()
        var cur = ref
        cur[8] = CGPoint(x: cur[8].x + 0.12, y: cur[8].y + 0.08)
        let result = RigidHandChecker.evaluate(current: cur, reference: ref)
        XCTAssertFalse(result.accepted)
    }

    func testOrbitSessionCoverage() {
        var session = OrbitCaptureSession(handSide: "left")
        let intrinsics = CameraIntrinsics.fromFieldOfView(imageWidth: 1920, imageHeight: 1080, fovHorizontalDeg: 68)
        let lm = PalmRegionGeometry.scaffoldLandmarks().map { [Float($0.x), Float($0.y)] }
        session.frames.append(OrbitFrame(
            bin: .flatCenter,
            timestamp: 1,
            landmarks: lm,
            gravity: [0, 0, -1],
            deviceRollDeg: 0,
            intrinsics: intrinsics,
            imageWidth: 1920,
            imageHeight: 1080,
            digitSpreadOk: true,
            maxJointDrift: 0.01
        ))
        XCTAssertEqual(session.filledBins.count, 1)
        XCTAssertGreaterThan(session.coverageFraction, 0)
    }

    func testMountReliefEstimatorProducesVenusEntry() {
        var session = OrbitCaptureSession(handSide: "right")
        let intrinsics = CameraIntrinsics.fromFieldOfView(imageWidth: 1920, imageHeight: 1080, fovHorizontalDeg: 68)
        let lm = PalmRegionGeometry.scaffoldLandmarks().map { [Float($0.x), Float($0.y)] }
        for bin in OrbitBin.allCases {
            session.frames.append(OrbitFrame(
                bin: bin,
                timestamp: Double(bin.hashValue),
                landmarks: lm,
                gravity: [0.1, 0.2, -0.9],
                deviceRollDeg: bin == .tiltLeft ? 18 : 0,
                intrinsics: intrinsics,
                imageWidth: 1920,
                imageHeight: 1080,
                digitSpreadOk: true,
                maxJointDrift: 0.01
            ))
        }
        let map = MountReliefEstimator.estimate(from: session)
        XCTAssertFalse(map.mounts.isEmpty)
        XCTAssertNotNil(map.entry(for: .venus))
    }

    func testCameraIntrinsicsJsonRoundTripFields() {
        let i = CameraIntrinsics.fromFieldOfView(imageWidth: 4032, imageHeight: 3024, fovHorizontalDeg: 68)
        XCTAssertEqual(i.imageWidth, 4032)
        XCTAssertGreaterThan(i.focalLengthX, 100)
    }
}
