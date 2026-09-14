import AVFoundation
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

    func testGuidanceCapturesReferenceGravity() {        let guidance = OrbitCaptureGuidance(handSide: "left")
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

    // MARK: - Hardware depth path (simulator-safe; no camera required)

    func testDepthProbeFallsBackWithoutHardware() {
        let none = DepthCapability.probe(photoOutput: nil)
        XCTAssertFalse(none.supported)
        XCTAssertFalse(none.photoDepthSupported)
        // A detached photo output on Simulator supports no depth delivery.
        let bare = DepthCapability.probe(photoOutput: AVCapturePhotoOutput())
        XCTAssertFalse(bare.photoDepthSupported)
        XCTAssertFalse(bare.supported)
    }

    /// Synthetic 64x64 gradient map (300 mm left -> 400 mm right) sampled at
    /// the scaffold landmarks: each mount must read ~its anchor position.
    func testMountDepthSamplerReadsGradient() {
        let w = 64
        let h = 64
        var meters: [Float] = []
        meters.reserveCapacity(w * h)
        for y in 0 ..< h {
            for x in 0 ..< w {
                _ = y
                meters.append(0.30 + 0.10 * Float(x) / Float(w - 1))
            }
        }
        let depth = PhotoDepthMap(width: w, height: h, meters: meters, source: "test", quality: 1)
        XCTAssertTrue(depth.isValid)
        let lm = PalmRegionGeometry.scaffoldLandmarks().map { [Float($0.x), Float($0.y)] }
        guard let means = MountDepthSampler.sampleMountMeansMm(depthMap: depth, landmarks: lm) else {
            XCTFail("expected 7 valid mount means")
            return
        }
        XCTAssertEqual(means.count, 7)
        for (mount, idxs) in MountDepthSampler.anchors {
            let meanX = idxs.map { lm[$0][0] }.reduce(0, +) / Float(idxs.count)
            let expected = 300 + 100 * meanX
            XCTAssertEqual(means[mount] ?? -1, expected, accuracy: 15,
                           "mount \(mount) should track its anchor position")
        }
    }

    func testMountDepthSamplerRejectsGarbage() {
        let lm = PalmRegionGeometry.scaffoldLandmarks().map { [Float($0.x), Float($0.y)] }
        let nan = PhotoDepthMap(width: 32, height: 32,
                                meters: [Float](repeating: .nan, count: 32 * 32),
                                source: "test", quality: 0)
        XCTAssertNil(MountDepthSampler.sampleMountMeansMm(depthMap: nan, landmarks: lm))
        // Background only (5 m wall behind the hand) is rejected too.
        let far = PhotoDepthMap(width: 32, height: 32,
                                meters: [Float](repeating: 5.0, count: 32 * 32),
                                source: "test", quality: 0)
        XCTAssertNil(MountDepthSampler.sampleMountMeansMm(depthMap: far, landmarks: lm))
        // Fewer than 21 landmarks cannot be probed.
        let ok = PhotoDepthMap(width: 32, height: 32,
                               meters: [Float](repeating: 0.4, count: 32 * 32),
                               source: "test", quality: 1)
        XCTAssertNil(MountDepthSampler.sampleMountMeansMm(depthMap: ok, landmarks: Array(lm.prefix(10))))
    }

    func testEstimatorUsesMeasuredDepth() {
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
            maxJointDrift: 0.01,
            depthMm: ["jupiter": 300, "saturn": 300, "apollo": 300, "mercury": 300,
                      "venus": 320, "luna": 300, "mars": 300],
            depthSource: "lidar_avdepth_v1"
        ))
        let map = MountReliefEstimator.estimate(from: session)
        XCTAssertEqual(map.source, "orbit_depth_v1")
        let venus = map.mounts.first { $0.mount == "venus" }
        XCTAssertEqual(venus?.heightMm ?? -1, 20.0, accuracy: 0.01)
        XCTAssertEqual(venus?.prominence, .elevated)
    }

    func testDepthQualityAndValidation() {
        let validMap = PhotoDepthMap(width: 4, height: 4, meters: [Float](repeating: 0.35, count: 16),
                                     source: "lidar_avdepth_v1", quality: 1)
        XCTAssertTrue(validMap.isValid)
        XCTAssertEqual(validMap.quality, 1)
        XCTAssertEqual(validMap.source, "lidar_avdepth_v1")

        let invalidMap = PhotoDepthMap(width: 4, height: 4, meters: [Float](repeating: 0.35, count: 15),
                                       source: "disparity_avdepth_v1", quality: 0)
        XCTAssertFalse(invalidMap.isValid)
        XCTAssertEqual(invalidMap.quality, 0)
    }

    // MARK: - Hand-measured orbit gates (phone stays still)

    private func orbitPose(for landmarks: [CGPoint]) -> SubjectPose {
        let cx = landmarks.map(\.x).reduce(0, +) / CGFloat(landmarks.count)
        let cy = landmarks.map(\.y).reduce(0, +) / CGFloat(landmarks.count)
        return SubjectPose(centroid: CGPoint(x: cx, y: cy), scale: 0.45,
                           rollDeg: 0, aspectRatio: 1)
    }

    private func orbitPhoto() -> PhotometricSample {
        PhotometricSample(meanLuminance: 150, glareRatio: 0)
    }

    private func orbitFrame(bin: OrbitBin, landmarks: [CGPoint], at t: TimeInterval) -> OrbitFrame {
        let intrinsics = CameraIntrinsics.fromFieldOfView(imageWidth: 1000, imageHeight: 1000, fovHorizontalDeg: 68)
        return OrbitFrame(
            bin: bin, timestamp: t,
            landmarks: landmarks.map { [Float($0.x), Float($0.y)] },
            gravity: [0, 0, -1], deviceRollDeg: 0, intrinsics: intrinsics,
            imageWidth: 1000, imageHeight: 1000,
            digitSpreadOk: true, maxJointDrift: 0.01
        )
    }

    /// Rotate landmarks in-plane about their centroid (simulates a hand tilt
    /// with the phone held perfectly still — device roll stays 0 throughout).
    private func rotated(_ landmarks: [CGPoint], degrees: CGFloat) -> [CGPoint] {
        let cx = landmarks.map(\.x).reduce(0, +) / CGFloat(landmarks.count)
        let cy = landmarks.map(\.y).reduce(0, +) / CGFloat(landmarks.count)
        let r = degrees * .pi / 180
        return landmarks.map { p in
            let dx = p.x - cx
            let dy = p.y - cy
            return CGPoint(x: cx + dx * cos(r) - dy * sin(r),
                           y: cy + dx * sin(r) + dy * cos(r))
        }
    }

    /// Flat fills, then a 20° hand tilt (phone steady) must reach `capturing`
    /// for tiltLeft — never `coach` (no device tilt required) and never
    /// `rejected` (rigidity reference refreshes per bin).
    func testOrbitTiltFillsWithSteadyPhone() {
        let guidance = OrbitCaptureGuidance(handSide: "left")
        let flat = PalmRegionGeometry.scaffoldLandmarks()
        let tilted = rotated(flat, degrees: 20)

        // Bin 1: hold flat 0.6 s -> keyframe.
        var progressed = false
        for t in [0.0, 0.3, 0.6] {
            let eval = guidance.evaluate(pose: orbitPose(for: flat), landmarks: flat,
                                         photometrics: orbitPhoto(), timestamp: t)
            if eval.shouldCapture {
                progressed = true
                if case .capturing(let b) = eval.state {
                    guidance.recordFrame(orbitFrame(bin: b, landmarks: flat, at: t))
                }
            }
        }
        XCTAssertTrue(progressed, "flatCenter should capture after a 0.6 s hold")
        XCTAssertTrue(guidance.orbitSession.filledBins.contains(.flatCenter))

        // Bin 2: tilt the HAND 20° with the phone steady. Must not stall.
        var sawHold = false
        var captured = false
        for t in [1.0, 1.3, 1.6] {
            let eval = guidance.evaluate(pose: orbitPose(for: tilted), landmarks: tilted,
                                         photometrics: orbitPhoto(), timestamp: t)
            if case .rejected = eval.state {
                XCTFail("tilted hand must not be rejected once the bin reference refreshes")
            }
            if case .coach = eval.state {
                XCTFail("tilt gate is hand-measured — a 20° tilt must pass without moving the phone")
            }
            if case .holdSteady = eval.state { sawHold = true }
            if eval.shouldCapture {
                captured = true
                if case .capturing(let b) = eval.state {
                    guidance.recordFrame(orbitFrame(bin: b, landmarks: tilted, at: t))
                }
            }
        }
        XCTAssertTrue(sawHold, "tilted hold should report holdSteady progress")
        XCTAssertTrue(captured, "tiltLeft should capture after a 0.6 s tilted hold")
        XCTAssertTrue(guidance.orbitSession.filledBins.contains(.tiltLeft))
    }

    /// Holding the flat pose during a tilt bin must keep coaching (the gate
    /// really measures the hand), not silently pass.
    func testOrbitTiltGateBlocksFlatPose() {
        let guidance = OrbitCaptureGuidance(handSide: "right")
        let flat = PalmRegionGeometry.scaffoldLandmarks()
        for t in [0.0, 0.3, 0.6] {
            let eval = guidance.evaluate(pose: orbitPose(for: flat), landmarks: flat,
                                         photometrics: orbitPhoto(), timestamp: t)
            if eval.shouldCapture, case .capturing(let b) = eval.state {
                guidance.recordFrame(orbitFrame(bin: b, landmarks: flat, at: t))
            }
        }
        XCTAssertTrue(guidance.orbitSession.filledBins.contains(.flatCenter))
        let eval = guidance.evaluate(pose: orbitPose(for: flat), landmarks: flat,
                                     photometrics: orbitPhoto(), timestamp: 2.0)
        if case .coach(let bin) = eval.state {
            XCTAssertEqual(bin, .tiltLeft)
        } else {
            XCTFail("flat pose during tiltLeft must coach, got \(eval.state)")
        }
        XCTAssertFalse(eval.shouldCapture)
    }

    /// Displayed progress never sits at 0% once work starts: a finished FLAT
    /// step reads 1/6 immediately, and holding the current angle climbs
    /// gradually toward the next step.
    func testDisplayProgressCreditsFinishedAndActiveBins() {
        var session = OrbitCaptureSession(handSide: "left")
        XCTAssertEqual(session.displayProgress(for: .idle), 0, accuracy: 0.001)

        let flat = PalmRegionGeometry.scaffoldLandmarks().map { [Float($0.x), Float($0.y)] }
        session.frames.append(orbitFrame(bin: .flatCenter,
                                         landmarks: flat.map { CGPoint(x: CGFloat($0[0]), y: CGFloat($0[1])) },
                                         at: 1))
        XCTAssertEqual(session.displayProgress(for: .idle), 1.0 / 6.0, accuracy: 0.001)

        let holding = session.displayProgress(for: .holdSteady(bin: .tiltLeft, progress: 0.5))
        XCTAssertGreaterThan(holding, 1.0 / 6.0, "holding the active angle must climb past the last finished step")
        XCTAssertLessThan(holding, 2.0 / 6.0, "a half hold must not credit the full next step")
    }

    /// Setup blockers coach specifically instead of a dead "searching":
    /// visible-but-unframed hand -> fitHand, tiny hand -> moveCloser,
    /// dark frame -> addLight.
    func testOrbitSetupBlockersCoachSpecifically() {
        let guidance = OrbitCaptureGuidance(handSide: "left")
        let flat = PalmRegionGeometry.scaffoldLandmarks()

        let noPose = guidance.evaluate(pose: nil, landmarks: flat,
                                       photometrics: orbitPhoto(), timestamp: 0)
        if case .fitHand = noPose.state {} else {
            XCTFail("hand without usable framing must ask for fitHand, got \(noPose.state)")
        }

        var tiny = orbitPose(for: flat)
        tiny.scale = 0.1
        let far = guidance.evaluate(pose: tiny, landmarks: flat,
                                    photometrics: orbitPhoto(), timestamp: 0)
        if case .moveCloser = far.state {} else {
            XCTFail("tiny hand must ask to moveCloser, got \(far.state)")
        }

        let dark = guidance.evaluate(pose: orbitPose(for: flat), landmarks: flat,
                                     photometrics: PhotometricSample(meanLuminance: 5, glareRatio: 0),
                                     timestamp: 0)
        if case .addLight = dark.state {} else {
            XCTFail("dark frame must ask to addLight, got \(dark.state)")
        }
    }
}
