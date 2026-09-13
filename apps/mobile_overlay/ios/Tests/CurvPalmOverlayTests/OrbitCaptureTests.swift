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

    func testMetricEstimatorReportsMillimeters() {
        var session = OrbitCaptureSession(handSide: "right")
        let intrinsics = CameraIntrinsics.fromFieldOfView(imageWidth: 1000, imageHeight: 1000, fovHorizontalDeg: 68)
        let ref = PalmRegionGeometry.scaffoldLandmarks().map { [Float($0.x), Float($0.y)] }
        var tilted = ref
        for idx in [1, 2, 5] {
            tilted[idx][0] -= 0.012
        }
        session.referenceLandmarks = ref
        session.referenceGravity = [0, 0, -1]
        session.frames.append(OrbitFrame(
            bin: .flatCenter,
            timestamp: 1,
            landmarks: ref,
            gravity: [0, 0, -1],
            deviceRollDeg: 0,
            intrinsics: intrinsics,
            imageWidth: 1000,
            imageHeight: 1000,
            digitSpreadOk: true,
            maxJointDrift: 0.01
        ))
        session.frames.append(OrbitFrame(
            bin: .tiltLeft,
            timestamp: 2,
            landmarks: tilted,
            gravity: [0.3, 0.1, -0.94],
            deviceRollDeg: 18,
            intrinsics: intrinsics,
            imageWidth: 1000,
            imageHeight: 1000,
            digitSpreadOk: true,
            maxJointDrift: 0.01
        ))
        let map = MountReliefEstimator.estimate(from: session)
        XCTAssertEqual(map.source, "orbit_metric_v1")
        let venus = map.mounts.first { $0.mount == "venus" }
        let luna = map.mounts.first { $0.mount == "luna" }
        XCTAssertNotNil(venus)
        XCTAssertNotNil(venus?.heightMm)
        XCTAssertGreaterThan(venus?.heightMm ?? 0, 0)
        XCTAssertGreaterThan((venus?.heightMm ?? 0) - (luna?.heightMm ?? -10), 1.0)
        XCTAssertEqual(venus?.prominence, .elevated)
        XCTAssertTrue(map.venusElevated)
    }

    func testLegacySessionStaysProxyWithoutMillimeters() {
        var session = OrbitCaptureSession(handSide: "right")
        let intrinsics = CameraIntrinsics.fromFieldOfView(imageWidth: 1000, imageHeight: 1000, fovHorizontalDeg: 68)
        let lm = PalmRegionGeometry.scaffoldLandmarks().map { [Float($0.x), Float($0.y)] }
        session.frames.append(OrbitFrame(
            bin: .flatCenter,
            timestamp: 1,
            landmarks: lm,
            gravity: [0, 0, -1],
            deviceRollDeg: 0,
            intrinsics: intrinsics,
            imageWidth: 1000,
            imageHeight: 1000,
            digitSpreadOk: true,
            maxJointDrift: 0.01
        ))
        let map = MountReliefEstimator.estimate(from: session)
        XCTAssertEqual(map.source, "orbit_tilt_proxy_v1")
        XCTAssertNil(map.mounts.first { $0.mount == "venus" }?.heightMm)
    }

    func testOrbitSessionUsesSnakeCaseKeys() throws {
        var session = OrbitCaptureSession(handSide: "left")
        session.referenceGravity = [0, 0, -1]
        let data = try session.toJSONData()
        let json = String(data: data, encoding: .utf8) ?? ""
        XCTAssertTrue(json.contains("hand_side"))
        XCTAssertTrue(json.contains("reference_gravity"))
        XCTAssertTrue(json.contains("started_at"))
    }

    func testGuidanceCapturesReferenceGravity() {
        let guidance = OrbitCaptureGuidance(handSide: "left")
        XCTAssertNil(guidance.orbitSession.referenceGravity)
        let intrinsics = CameraIntrinsics.fromFieldOfView(imageWidth: 1000, imageHeight: 1000, fovHorizontalDeg: 68)
        let lm = PalmRegionGeometry.scaffoldLandmarks().map { [Float($0.x), Float($0.y)] }
        guidance.recordFrame(OrbitFrame(
            bin: .flatCenter,
            timestamp: 1,
            landmarks: lm,
            gravity: [0.1, 0.2, -0.97],
            deviceRollDeg: 0,
            intrinsics: intrinsics,
            imageWidth: 1000,
            imageHeight: 1000,
            digitSpreadOk: true,
            maxJointDrift: 0.01
        ))
        XCTAssertEqual(guidance.orbitSession.referenceGravity ?? [], [0.1, 0.2, -0.97])
    }
}
