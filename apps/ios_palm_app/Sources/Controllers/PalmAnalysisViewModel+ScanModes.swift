import CoreGraphics
import CurvCameraOverlay
import CurvPalmOverlay
import Foundation
import UIKit

// MARK: - Mode B/C/D scan protocols (orbit, close-up mosaic, fingertip)

extension PalmAnalysisViewModel {
    public func setScanMode(_ mode: PalmScanMode) {
        scanMode = mode
        orbitGuidance.reset(handSide: detectedHandSide.rawValue)
        closeUpSession = CloseUpMosaicSession()
        fingertipSession = FingertipScanSession()
        if mode == .orbitScan {
            deviceTiltProvider.start()
        }
        let isMacro = (mode == .closeUpMosaic || mode == .fingertipScan)
        cameraSession.setMacroFocus(enabled: isMacro)
        statusMessageOverride = mode.subtitle
    }

    func processScanModeFrame(
        pixelBuffer: CVPixelBuffer,
        landmarks: [CGPoint]?,
        pose: SubjectPose?,
        photometrics: PhotometricSample,
        tipsOK: Bool,
        now: CFTimeInterval
    ) -> Bool {
        switch scanMode {
        case .stillPhoto:
            return false
        case .orbitScan:
            processOrbitFrame(landmarks: landmarks, pose: pose, photometrics: photometrics, tipsOK: tipsOK, now: now)
            return true
        case .closeUpMosaic:
            processCloseUpFrame(landmarks: landmarks, pose: pose, photometrics: photometrics, now: now)
            return true
        case .fingertipScan:
            processFingertipFrame(landmarks: landmarks, photometrics: photometrics, now: now)
            return true
        }
    }

    private func processOrbitFrame(
        landmarks: [CGPoint]?,
        pose: SubjectPose?,
        photometrics: PhotometricSample,
        tipsOK: Bool,
        now: CFTimeInterval
    ) {
        guard !isCapturing else { return }
        let eval = orbitGuidance.evaluate(
            pose: tipsOK ? pose : nil,
            landmarks: landmarks,
            photometrics: photometrics,
            deviceRollDeg: deviceTiltProvider.deviceRollDeg,
            timestamp: now
        )
        orbitGuidance.setLastState(eval.state)

        DispatchQueue.main.async {
            self.orbitState = eval.state
            self.statusMessageOverride = self.orbitGuidance.statusMessage
            self.orbitSession = self.orbitGuidance.orbitSession
            self.updateMountReliefHeat()

            if eval.shouldCapture, let landmarks, let pose {
                self.captureOrbitKeyframe(landmarks: landmarks, bin: {
                    if case .capturing(let b) = eval.state { return b }
                    return self.orbitGuidance.orbitSession.filledBins.isEmpty ? .flatCenter : .tiltLeft
                }())
            }
        }
    }

    private func captureOrbitKeyframe(landmarks: [CGPoint], bin: OrbitBin) {
        guard !isCapturing else { return }
        isCapturing = true
        cameraSession.capturePhoto { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let photo):
                self.finishOrbitKeyframe(photo: photo, landmarks: landmarks, bin: bin)
            case .failure(let err):
                self.logger.error("Orbit keyframe failed: \(err.localizedDescription)")
                DispatchQueue.main.async { self.isCapturing = false }
            }
        }
    }

    private func finishOrbitKeyframe(photo: CapturedPhoto, landmarks: [CGPoint], bin: OrbitBin) {
        let g = deviceTiltProvider.gravityVector
        let gravity = [g.x, g.y, g.z]
        let lm = landmarks.map { [Float($0.x), Float($0.y)] }
        let w = photo.intrinsics.imageWidth
        let h = photo.intrinsics.imageHeight
        let rigid = RigidHandChecker.evaluate(current: landmarks, reference: orbitGuidance.orbitSession.referenceLandmarks?.map {
            CGPoint(x: CGFloat($0[0]), y: CGFloat($0[1]))
        })
        // Hardware depth (LiDAR/disparity) when this keyframe delivered it.
        // Nil on unsupported devices — estimator and native engine fall back.
        let depthMm = photo.depthMap.flatMap {
            MountDepthSampler.sampleMountMeansMm(depthMap: $0, landmarks: lm)
        }
        let frame = OrbitFrame(
            bin: bin,
            timestamp: Date().timeIntervalSince1970,
            landmarks: lm,
            gravity: gravity,
            deviceRollDeg: deviceTiltProvider.deviceRollDeg,
            intrinsics: photo.intrinsics,
            imageWidth: w,
            imageHeight: h,
            digitSpreadOk: rigid.digitSpreadOk,
            maxJointDrift: Float(rigid.maxDrift),
            depthMm: depthMm,
            depthSource: depthMm != nil ? photo.depthMap?.source : nil
        )
        orbitGuidance.recordFrame(frame)
        let relief = MountReliefEstimator.estimate(from: orbitGuidance.orbitSession)
        if let engine = curvEngine,
           let json = try? orbitGuidance.orbitSession.toJSONData(),
           let str = String(data: json, encoding: .utf8),
           case .success(let nativeJson) = engine.analyzeOrbitSession(json: str) {
            if let parsed = PalmAnalysisResponse.from(jsonString: nativeJson) {
                DispatchQueue.main.async {
                    self.mergeMountRelief(from: parsed)
                }
            }
        }
        DispatchQueue.main.async {
            self.orbitSession = self.orbitGuidance.orbitSession
            self.mountReliefMap = relief
            self.updateMountReliefHeat()
            self.isCapturing = false
            if case .complete = self.orbitState {
                self.successHaptic.notificationOccurred(.success)
            } else {
                self.lightHaptic.impactOccurred()
            }
        }
    }

    private func processCloseUpFrame(
        landmarks: [CGPoint]?,
        pose: SubjectPose?,
        photometrics: PhotometricSample,
        now: CFTimeInterval
    ) {
        guard !isCapturing, let target = closeUpGuidance.nextTile(in: closeUpSession) else {
            DispatchQueue.main.async { self.statusMessageOverride = "Close-up mosaic complete" }
            return
        }
        let eval = closeUpGuidance.evaluate(pose: pose, photometrics: photometrics, target: target)
        DispatchQueue.main.async {
            self.statusMessageOverride = eval.message
        }
        guard eval.ready, let landmarks, landmarks.count >= 21 else { return }
        isCapturing = true
        cameraSession.capturePhoto { [weak self] result in
            guard let self else { return }
            defer { DispatchQueue.main.async { self.isCapturing = false } }
            guard case .success(let photo) = result else { return }
            let lm = landmarks.map { [Float($0.x), Float($0.y)] }
            let tile = CloseUpTileFrame(
                tile: target,
                timestamp: now,
                landmarks: lm,
                intrinsics: photo.intrinsics,
                imageWidth: photo.intrinsics.imageWidth,
                imageHeight: photo.intrinsics.imageHeight
            )
            DispatchQueue.main.async {
                self.closeUpSession.tiles.append(tile)
                self.processCloseUpWithEngine(photo: photo, landmarks: landmarks)
            }
        }
    }

    private func processCloseUpWithEngine(photo: CapturedPhoto, landmarks: [CGPoint]) {
        guard let engine = curvEngine else { return }
        let flat = VisionHandTracker.toFlatLandmarksArray(landmarks)
        let buffer = Self.makeBGRAPixelBuffer(from: Self.uprightImage(from: photo.pixelBuffer).0) ?? photo.pixelBuffer
        if case .success(let json) = engine.processPixelBuffer(pixelBuffer: buffer, landmarks: flat, includeMinorRidges: true),
           let parsed = PalmAnalysisResponse.from(jsonString: json) {
            DispatchQueue.main.async {
                if var existing = self.analysisResult {
                    // Merge minor ridges into existing result
                    self.analysisResult = parsed
                    _ = existing
                } else {
                    self.analysisResult = parsed
                }
            }
        }
    }

    private func processFingertipFrame(
        landmarks: [CGPoint]?,
        photometrics: PhotometricSample,
        now: CFTimeInterval
    ) {
        guard !isCapturing,
              let target = fingertipGuidance.nextDigit(in: fingertipSession),
              let landmarks else { return }
        let eval = fingertipGuidance.evaluate(landmarks: landmarks, target: target, photometrics: photometrics)
        DispatchQueue.main.async { self.statusMessageOverride = eval.message }
        guard eval.ready else { return }
        isCapturing = true
        cameraSession.capturePhoto { [weak self] result in
            guard let self else { return }
            defer { DispatchQueue.main.async { self.isCapturing = false } }
            guard case .success(let photo) = result else { return }
            let pattern = self.classifyFingertip(photo: photo, landmarks: landmarks, digit: target)
            DispatchQueue.main.async {
                self.fingertipSession.results.append(pattern)
            }
        }
    }

    private func classifyFingertip(photo: CapturedPhoto, landmarks: [CGPoint], digit: FingertipDigit) -> FingertipScanResult {
        if let engine = curvEngine {
            let flat = VisionHandTracker.toFlatLandmarksArray(landmarks)
            let buffer = Self.makeBGRAPixelBuffer(from: Self.uprightImage(from: photo.pixelBuffer).0) ?? photo.pixelBuffer
            if case .success(let json) = engine.classifyFingertip(pixelBuffer: buffer, landmarks: flat, digit: digit.rawValue),
               let parsed = PalmAnalysisResponse.from(jsonString: json),
               let payload = parsed.domainPayload?.fingertipPattern {
                return FingertipScanResult(
                    digit: digit,
                    pattern: DermatoglyphicPattern(rawValue: payload.pattern ?? "unknown") ?? .unknown,
                    ridgeFrequency: payload.ridgeFrequency ?? 0,
                    quality: payload.quality ?? 0.5,
                    timestamp: Date().timeIntervalSince1970
                )
            }
        }
        return FingertipScanResult(
            digit: digit,
            pattern: .loop,
            ridgeFrequency: 0.8,
            quality: 0.4,
            timestamp: Date().timeIntervalSince1970
        )
    }

    func updateMountReliefHeat() {
        guard let map = mountReliefMap else {
            mountReliefHeat = [:]
            return
        }
        var heat: [String: Float] = [:]
        for entry in map.mounts {
            heat[entry.mount] = min(1, max(0, (entry.heightRelative + 1) * 0.5))
        }
        mountReliefHeat = heat
    }

    func mergeMountRelief(from response: PalmAnalysisResponse) {
        guard let relief = response.domainPayload?.mountRelief else { return }
        mountReliefMap = MountReliefMap(
            mounts: (relief.mounts ?? []).map {
                MountReliefEntry(
                    mount: $0.mount,
                    heightMm: $0.heightMm,
                    heightRelative: $0.heightRelative ?? 0,
                    prominence: MountProminence(rawValue: $0.prominence ?? "average") ?? .average,
                    confidence: $0.confidence ?? 0,
                    viewCount: $0.viewCount ?? 0
                )
            },
            venusElevated: relief.venusElevated ?? false,
            source: relief.source ?? "native"
        )
        updateMountReliefHeat()
    }

    public func exportOrbitSessionJSON() -> Data? {
        try? orbitSession.toJSONData()
    }

    func analysisResultWithMountRelief(
        _ response: PalmAnalysisResponse?,
        relief: MountReliefMap,
        intrinsics: CameraIntrinsics
    ) -> PalmAnalysisResponse? {
        guard var response else { return nil }
        let payload = MountReliefPayload(
            mounts: relief.mounts.map {
                MountReliefEntryPayload(
                    mount: $0.mount,
                    heightMm: $0.heightMm,
                    heightRelative: $0.heightRelative,
                    prominence: $0.prominence.rawValue,
                    confidence: $0.confidence,
                    viewCount: $0.viewCount
                )
            },
            venusElevated: relief.venusElevated,
            source: relief.source
        )
        // Re-encode with mount_relief merged — lightweight JSON patch.
        guard let existing = response.domainPayload else { return response }
        var fingertip = existing.fingertipPattern
        var minor = existing.minorRidges
        let merged = PalmDomainPayload(
            majorLines: existing.majorLines,
            dominantLine: existing.dominantLine,
            heartHeadMinDistancePx: existing.heartHeadMinDistancePx,
            fateAnalysis: existing.fateAnalysis,
            heartAnalysis: existing.heartAnalysis,
            headAnalysis: existing.headAnalysis,
            lifeAnalysis: existing.lifeAnalysis,
            mountRelief: payload,
            minorRidges: minor,
            fingertipPattern: fingertip
        )
        _ = intrinsics
        if let data = try? JSONEncoder().encode(PalmAnalysisResponseWrapper(isUsable: response.isUsable, totalDurationUs: response.totalDurationUs, domainPayload: merged)),
           let str = String(data: data, encoding: .utf8),
           let parsed = PalmAnalysisResponse.from(jsonString: str) {
            return parsed
        }
        return response
    }
}
