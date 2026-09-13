import Foundation
import CoreGraphics
import CoreVideo
import UIKit
import Combine
import os.log
import CurvApple
import CurvCameraOverlay
import CurvPalmOverlay

/**
 * Palm-domain ViewModel. Camera / HUD come from the overlay kit.
 * Post-capture priority: keep the photo and draw crease overlays on it.
 * CurvEngine polylines are preferred when the XCFramework is linked;
 * landmark guides are fallback-only.
 */
public final class PalmAnalysisViewModel: ObservableObject {
    let logger = Logger(subsystem: "org.curv.palm", category: "PalmAnalysisViewModel")

    public let profile = PalmOverlay.profile
    public let cameraSession = CameraSession()
    private let handTracker = VisionHandTracker()
    private let guidanceEngine: CaptureGuidance
    let orbitGuidance = OrbitCaptureGuidance()
    let closeUpGuidance = CloseUpTileGuidance()
    let fingertipGuidance = FingertipCaptureGuidance()
    let deviceTiltProvider = DeviceTiltProvider()
    var curvEngine: CurvEngine?

    @Published public var scanMode: PalmScanMode = .stillPhoto
    @Published public var orbitState: OrbitGuidanceState = .idle
    @Published public var orbitSession: OrbitCaptureSession = OrbitCaptureSession(handSide: "auto")
    @Published public var closeUpSession = CloseUpMosaicSession()
    @Published public var fingertipSession = FingertipScanSession()
    @Published public var mountReliefMap: MountReliefMap?
    @Published public var mountReliefHeat: [String: Float] = [:]

    @Published public var guidanceState: GuidanceState = .searching
    @Published public var qualityMetrics = QualityMetrics()
    @Published public var statusMessageOverride: String?
    @Published public var detectedLandmarks: [CGPoint] = []
    @Published public var isAnalyzing: Bool = false
    @Published public var analysisResult: PalmAnalysisResponse?
    public var padDepths: [String: Float] {
        guard let mounts = analysisResult?.domainPayload?.mountRelief?.mounts else { return [:] }
        var dict: [String: Float] = [:]
        for m in mounts {
            if let h = m.heightMm {
                dict[m.mount] = h
            }
        }
        return dict
    }
    @Published public var detectedSplines: [CreaseSpline] = []
    @Published public var captureLandmarks: [CGPoint] = []
    @Published public var capturedImage: UIImage?
    /// Multi-shot capture session (e.g. left palm + right palm today; a future
    /// angle like the back of the hand just needs `requiredShotCount` bumped).
    @Published public private(set) var capturedShots: [UIImage] = []
    /// Shots this session needs before the filmstrip badge reads "complete".
    public var requiredShotCount: Int = 2
    /// Increments on every successful capture. UIImage isn't Equatable, so the
    /// filmstrip badge watches this (not `capturedShots`) to trigger its
    /// landing/shake animation exactly once per new shot.
    @Published public private(set) var captureTick: Int = 0
    public var isCaptureSessionComplete: Bool { capturedShots.count >= requiredShotCount }
    /// Full-screen overlay review (opened from the camera-roll style thumbnail).
    @Published public var showResultViewer: Bool = false
    @Published public var usedNativeEngine: Bool = false
    @Published public var overlaySource: CreaseOverlaySource = .landmarkFallback
    /// Adjusted lines ready for prediction / VLM (set by "Use for reading").
    @Published public var predictionReadySplines: [CreaseSpline] = []
    /// Dedicated Phase 3 Samudrika reading presentation sheet
    @Published public var showAnalysisReading: Bool = false
    @Published public var selectedHandSide: HandSide = .auto
    @Published public var detectedHandSide: HandSide = .right
    @Published public var currentLanguage: AppLanguage = .english {
        didSet {
            LocalizedStrings.currentLanguage = currentLanguage
        }
    }

    var isCapturing: Bool = false
    /// Prevents immediate re-arming of the 3-2-1 auto-capture after a shot.
    private var suppressAutoCaptureUntil: CFTimeInterval = 0
    private var lastCountdownDigit: Int = 0
    private let hapticFeedback = UIImpactFeedbackGenerator(style: .medium)
    let lightHaptic = UIImpactFeedbackGenerator(style: .light)
    let successHaptic = UINotificationFeedbackGenerator()
    /// Throttle UI metric publishes so Lux/Glare/Tilt stay smooth (~15 Hz).
    private var lastMetricsPublish: CFTimeInterval = 0

    public init() {
        self.guidanceEngine = CaptureGuidance(thresholds: PalmOverlay.profile.thresholds)
        do {
            self.curvEngine = try CurvEngine(domainId: PalmOverlay.profile.domainId)
            logger.info("CurvEngine native pipeline ready (isNativeAvailable=\(CurvEngine.isNativeAvailable))")
        } catch {
            logger.notice(
                "CurvEngine unavailable (isNativeAvailable=\(CurvEngine.isNativeAvailable), \(error.localizedDescription)) — landmark crease fallback only"
            )
        }

        setupCameraPipeline()
        hapticFeedback.prepare()
        lightHaptic.prepare()
        successHaptic.prepare()
    }

    private func setupCameraPipeline() {
        cameraSession.onFrameReceived = { [weak self] pixelBuffer in
            self?.processVideoFrame(pixelBuffer)
        }
    }

    public func start() {
        guidanceEngine.reset()
        cameraSession.start()
    }

    public func stop() {
        cameraSession.stop()
    }

    public func openLastCapture() {
        guard capturedImage != nil else { return }
        cameraSession.stop()
        showResultViewer = true
    }

    public func dismissResultViewer() {
        showResultViewer = false
        showAnalysisReading = false
        cameraSession.start()
    }

    public func clearLastCapture() {
        DispatchQueue.main.async {
            self.analysisResult = nil
            self.detectedSplines.removeAll()
            self.predictionReadySplines.removeAll()
            self.captureLandmarks.removeAll()
            self.capturedImage = nil
            if !self.capturedShots.isEmpty { self.capturedShots.removeLast() }
            self.showResultViewer = false
            self.showAnalysisReading = false
            self.usedNativeEngine = false
            self.overlaySource = .landmarkFallback
            self.cameraSession.start()
        }
    }

    public func retake() {
        DispatchQueue.main.async {
            self.isCapturing = false
            self.isAnalyzing = false
            self.analysisResult = nil
            self.detectedSplines.removeAll()
            self.predictionReadySplines.removeAll()
            self.captureLandmarks.removeAll()
            self.capturedImage = nil
            // The shot under review was rejected — drop it from the session
            // (and its badge count) so retaking doesn't overcount.
            if !self.capturedShots.isEmpty { self.capturedShots.removeLast() }
            self.showResultViewer = false
            self.showAnalysisReading = false
            self.usedNativeEngine = false
            self.overlaySource = .landmarkFallback
            self.guidanceEngine.reset()
            self.cameraSession.start()
        }
    }

    /// Starts a fresh multi-shot session (e.g. after submitting left+right for
    /// analysis, or the user explicitly restarts). Does not touch the current
    /// single-shot review state.
    public func resetCaptureSession() {
        capturedShots.removeAll()
        captureTick = 0
    }

    /// Proceed to Phase 3: lock lines and present Samudrika Reading Sheet
    public func proceedToAnalysis() {
        confirmAdjustedLinesForPrediction()
        mergeScanEnrichmentIntoAnalysisResult()
        showAnalysisReading = true
    }

    /// Attach orbit / close-up / fingertip scan data to the reading payload.
    public func mergeScanEnrichmentIntoAnalysisResult() {
        guard var response = analysisResult, let payload = response.domainPayload else {
            if let relief = mountReliefMap {
                analysisResult = analysisResultWithMountRelief(
                    analysisResult,
                    relief: relief,
                    intrinsics: CameraIntrinsics.fromFieldOfView(imageWidth: 1920, imageHeight: 1080, fovHorizontalDeg: 68)
                )
            }
            return
        }
        let reliefPayload: MountReliefPayload? = mountReliefMap.map { map in
            MountReliefPayload(
                mounts: map.mounts.map {
                    MountReliefEntryPayload(
                        mount: $0.mount,
                        heightMm: $0.heightMm,
                        heightRelative: $0.heightRelative,
                        prominence: $0.prominence.rawValue,
                        confidence: $0.confidence,
                        viewCount: $0.viewCount
                    )
                },
                venusElevated: map.venusElevated,
                source: map.source
            )
        }
        let tipPayload: FingertipPatternPayload? = fingertipSession.results.first.map {
            FingertipPatternPayload(
                digit: $0.digit.rawValue,
                pattern: $0.pattern.rawValue,
                ridgeFrequency: $0.ridgeFrequency,
                quality: $0.quality
            )
        }
        let merged = PalmDomainPayload(
            majorLines: payload.majorLines,
            dominantLine: payload.dominantLine,
            heartHeadMinDistancePx: payload.heartHeadMinDistancePx,
            fateAnalysis: payload.fateAnalysis,
            heartAnalysis: payload.heartAnalysis,
            headAnalysis: payload.headAnalysis,
            lifeAnalysis: payload.lifeAnalysis,
            mountRelief: reliefPayload ?? payload.mountRelief,
            minorRidges: payload.minorRidges,
            fingertipPattern: tipPayload ?? payload.fingertipPattern
        )
        if let data = try? JSONEncoder().encode(PalmAnalysisResponseWrapper(
            isUsable: response.isUsable,
            totalDurationUs: response.totalDurationUs,
            domainPayload: merged
        )), let str = String(data: data, encoding: .utf8) {
            analysisResult = PalmAnalysisResponse.from(jsonString: str)
        }
    }

    /// Lock current (possibly edited) lines as the prediction input.
    public func confirmAdjustedLinesForPrediction() {
        predictionReadySplines = detectedSplines
        logger.info("Prediction lines locked (\(self.predictionReadySplines.count) lines)")
    }

    /// JSON fragment for prediction grounded on user-visible / adjusted lines.
    public func predictionPayloadJSON() -> String? {
        let lines = predictionReadySplines.isEmpty ? detectedSplines : predictionReadySplines
        guard !lines.isEmpty else { return nil }
        var major: [String: Any] = [:]
        for line in lines {
            major[line.engineKey] = [
                "name": line.engineKey,
                "present": true,
                "confidence": line.confidence,
                "user_adjusted": line.isUserAdjusted,
                "points_image_norm": line.predictionPointsImageNorm
            ] as [String: Any]
        }
        let root: [String: Any] = [
            "domain_payload": [
                "major_lines": major,
                "source": overlaySource.rawValue
            ]
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: root, options: [.sortedKeys]),
              let str = String(data: data, encoding: .utf8)
        else { return nil }
        return str
    }

    private func processVideoFrame(_ pixelBuffer: CVPixelBuffer) {
        guard !isCapturing && !isAnalyzing && !showResultViewer else { return }

        let now = CACurrentMediaTime()
        if scanMode != .stillPhoto {
            let trackingResult = handTracker.processFrameWithResult(pixelBuffer)
            let landmarks = trackingResult?.landmarks
            let framing = landmarks.flatMap { HandLandmarkPose.framing(from: $0) }
            let pose = framing?.pose
            let tipsOK = framing?.isFullyInFrame ?? false
            let photoCenter = pose?.centroid ?? CGPoint(x: 0.5, y: 0.5)
            let photoRadius = pose.map { max(0.18, $0.scale * 0.28) } ?? 0.28
            let photometrics = Photometrics.sample(pixelBuffer: pixelBuffer, center: photoCenter, radius: photoRadius)
            if trackingResult != nil {
                DispatchQueue.main.async {
                    self.detectedLandmarks = trackingResult?.landmarks ?? []
                }
            }
            if processScanModeFrame(
                pixelBuffer: pixelBuffer,
                landmarks: landmarks,
                pose: pose,
                photometrics: photometrics,
                tipsOK: tipsOK,
                now: now
            ) {
                return
            }
        }
        let trackingResult = handTracker.processFrameWithResult(pixelBuffer)
        let landmarks = trackingResult?.landmarks
        let framing = landmarks.flatMap { HandLandmarkPose.framing(from: $0) }
        let pose = framing?.pose
        let tipsOK = framing?.isFullyInFrame ?? false

        let photoCenter = pose?.centroid ?? CGPoint(x: 0.5, y: 0.5)
        let photoRadius = pose.map { max(0.18, $0.scale * 0.28) } ?? 0.28
        let photometrics = Photometrics.sample(
            pixelBuffer: pixelBuffer,
            center: photoCenter,
            radius: photoRadius
        )

        // Gate countdown on soft fingertip margin; still coach with cm when scale is off.
        let gatePose = tipsOK ? pose : nil
        var evaluation = guidanceEngine.evaluate(
            pose: gatePose,
            photometrics: photometrics,
            timestamp: now
        )

        if let pose, !tipsOK {
            var metrics = evaluation.metrics
            metrics.scale = pose.scale
            metrics.centerOffset = {
                let dx = Float(pose.centroid.x - 0.5)
                let dy = Float(pose.centroid.y - 0.5)
                return sqrt(dx * dx + dy * dy)
            }()
            metrics.rollAngleDeg = abs(pose.rollDeg)
            metrics.flatnessRatio = pose.aspectRatio
            metrics.meanLuminance = photometrics.meanLuminance
            metrics.glareRatio = photometrics.glareRatio
            metrics.isIlluminationOk =
                photometrics.meanLuminance >= profile.thresholds.minLuminance &&
                photometrics.glareRatio <= profile.thresholds.maxGlareRatio
            metrics.isReadyForCapture = false
            evaluation = GuidanceEvaluation(state: .moveBack, metrics: metrics)
        }

        var statusOverride: String? = nil
        if let pose {
            if !tipsOK {
                statusOverride = "Move ~2 cm farther — keep fingertips inside"
            } else if let hint = PalmOverlay.distanceHintCm(scale: pose.scale, thresholds: profile.thresholds) {
                statusOverride = hint.closer
                    ? "Move ~\(hint.cm) cm closer"
                    : "Move ~\(hint.cm) cm farther"
            } else if case .centerSubject = evaluation.state {
                statusOverride = centerHint(for: pose)
            } else if case .ready(let progress) = evaluation.state, progress < 1 {
                statusOverride = profile.copy.message(for: evaluation.state)
            }
        }

        let shouldPublishMetrics = (now - lastMetricsPublish) >= (1.0 / 15.0)
        if shouldPublishMetrics {
            lastMetricsPublish = now
        }

        let autoArmed = now >= suppressAutoCaptureUntil
        let shouldCapture: Bool = {
            guard autoArmed,
                  capturedImage == nil,
                  landmarks != nil,
                  tipsOK,
                  case .ready(let progress) = evaluation.state,
                  progress >= 1.0,
                  evaluation.metrics.isReadyForCapture
            else { return false }
            return true
        }()

        DispatchQueue.main.async {
            if let trackingResult {
                self.detectedLandmarks = trackingResult.landmarks
                let newSide: HandSide = trackingResult.isLeftHand ? .left : .right
                if newSide != self.detectedHandSide {
                    self.detectedHandSide = newSide
                    self.lightHaptic.impactOccurred()
                }
            } else {
                self.detectedLandmarks.removeAll()
            }
            if !tipsOK {
                self.guidanceEngine.reset()
            }
            self.guidanceState = evaluation.state
            self.statusMessageOverride = statusOverride
            if shouldPublishMetrics {
                self.qualityMetrics = evaluation.metrics
            }

            if let digit = self.profile.copy.countdownDigit(for: evaluation.state), digit != self.lastCountdownDigit {
                self.lastCountdownDigit = digit
                self.lightHaptic.impactOccurred()
            }
            if case .ready = evaluation.state {
            } else {
                self.lastCountdownDigit = 0
            }

            if shouldCapture, let landmarks {
                self.triggerCapture(landmarks: landmarks)
            }
        }
    }

    private func centerHint(for pose: SubjectPose) -> String {
        let dx = Float(pose.centroid.x - 0.5)
        let dy = Float(pose.centroid.y - 0.5)
        let perCm: Float = 0.03
        if abs(dx) >= abs(dy) {
            let cm = max(1, Int(round(abs(dx) / perCm)))
            return dx > 0 ? "Move ~\(cm) cm left" : "Move ~\(cm) cm right"
        } else {
            let cm = max(1, Int(round(abs(dy) / perCm)))
            return dy > 0 ? "Move ~\(cm) cm up" : "Move ~\(cm) cm down"
        }
    }

    public func manualCapture() {
        guard !isCapturing && !isAnalyzing && !showResultViewer, !detectedLandmarks.isEmpty else { return }
        guard let framing = HandLandmarkPose.framing(from: detectedLandmarks), framing.isFullyInFrame else { return }
        triggerCapture(landmarks: detectedLandmarks)
    }

    private func triggerCapture(landmarks: [CGPoint]) {
        guard !isCapturing else { return }
        isCapturing = true
        // Block another auto cycle until this shot finishes + cooldown.
        suppressAutoCaptureUntil = CACurrentMediaTime() + 10.0

        DispatchQueue.main.async {
            self.hapticFeedback.impactOccurred()
            self.isAnalyzing = true
            self.guidanceState = .ready(countdownProgress: 1.0)
        }

        // Safety watchdog: ensure analysis loader can never hang the camera feed
        DispatchQueue.main.asyncAfter(deadline: .now() + 4.0) { [weak self] in
            guard let self, self.isAnalyzing else { return }
            self.logger.warning("Capture watchdog timeout reached — resetting analysis state")
            self.isAnalyzing = false
            self.isCapturing = false
            self.guidanceEngine.reset()
        }

        cameraSession.capturePhoto { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let captured):
                self.finishCapture(photoBuffer: captured.pixelBuffer, intrinsics: captured.intrinsics, landmarks: landmarks)
            case .failure(let error):
                self.logger.error("Photo capture failed: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.isCapturing = false
                    self.isAnalyzing = false
                    self.suppressAutoCaptureUntil = CACurrentMediaTime() + 1.5
                    self.guidanceEngine.reset()
                }
            }
        }
    }

    private func finishCapture(photoBuffer: CVPixelBuffer, intrinsics: CameraIntrinsics, landmarks videoLandmarks: [CGPoint]) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }

            // Bake an upright bitmap so display size == landmark space (no UIImage.orientation tricks).
            let (image, visionOrientation) = Self.uprightImage(from: photoBuffer)
            var photoLandmarks = videoLandmarks

            // Re-detect on the still itself — video landmarks live in a different FOV/crop.
            if let cgImage = image?.cgImage,
               let tracking = self.handTracker.processCGImageWithResult(cgImage, orientation: .up) {
                photoLandmarks = tracking.landmarks
                let stillSide: HandSide = tracking.isLeftHand ? .left : .right
                self.detectedHandSide = stillSide
                self.logger.info("Re-detected \(tracking.landmarks.count) landmarks on captured still (side: \(stillSide.rawValue))")
            } else if let tracking = self.handTracker.processFrameWithResult(photoBuffer, orientation: visionOrientation) {
                photoLandmarks = tracking.landmarks
                let stillSide: HandSide = tracking.isLeftHand ? .left : .right
                self.detectedHandSide = stillSide
                self.logger.info("Re-detected landmarks on photo buffer (side: \(stillSide.rawValue))")
            } else {
                self.logger.error("Still re-detect failed — falling back to video landmarks (likely misaligned)")
            }

            // Anatomical landmark scaffold: always provides all 4 major lines (Heart, Head, Life, Fate).
            let fallbackCreases = self.fallbackLandmarkCreases(landmarks: photoLandmarks)
            var splines = fallbackCreases
            var parsedResponse: PalmAnalysisResponse?
            var usedNative = false
            var source: CreaseOverlaySource = .landmarkFallback

            if let engine = self.curvEngine {
                // Process the upright bitmap so points_image_norm matches CaptureResultView.
                let processBuffer = Self.makeBGRAPixelBuffer(from: image) ?? photoBuffer
                let flat = VisionHandTracker.toFlatLandmarksArray(photoLandmarks)
                let includeMinor = self.scanMode == .closeUpMosaic
                switch engine.processPixelBuffer(pixelBuffer: processBuffer, landmarks: flat, includeMinorRidges: includeMinor) {
                case .success(let jsonString):
                    parsedResponse = PalmAnalysisResponse.from(jsonString: jsonString)
                    if parsedResponse == nil {
                        self.logger.error("Native JSON decode failed (len=\(jsonString.count)) — keeping landmark fallback")
                    } else if let nativeSplines = parsedResponse?.creaseSplinesFromPolylines(), !nativeSplines.isEmpty {
                        // Merge native lines with anatomical scaffold for any line missed by Steger
                        var merged: [CreaseSpline] = []
                        for fallback in fallbackCreases {
                            if let matchedNative = nativeSplines.first(where: { $0.lineType == fallback.lineType }) {
                                merged.append(matchedNative)
                            } else {
                                merged.append(fallback)
                            }
                        }
                        splines = merged
                        usedNative = true
                        source = .nativeEngine
                        self.logger.info("Native Steger detected lines; guaranteed all 4 major lines (Heart, Head, Life, Fate)")
                    } else {
                        let usable = parsedResponse?.isUsable.map(String.init) ?? "nil"
                        let lineCount = parsedResponse?.domainPayload?.majorLines?.count ?? 0
                        self.logger.notice(
                            "Native JSON had no usable points_image_norm (is_usable=\(usable), major_lines=\(lineCount)) — keeping landmark fallback"
                        )
                    }
                case .failure(let err):
                    self.logger.error("CurvEngine execution error: \(String(describing: err))")
                }
            } else {
                self.logger.notice("curvEngine is nil — landmark crease fallback only")
            }

            DispatchQueue.main.async {
                // Stay in live camera mode (iOS Camera style): keep thumbnail, resume preview.
                self.capturedImage = image
                if let image {
                    self.capturedShots.append(image)
                    self.captureTick += 1
                }
                self.captureLandmarks = photoLandmarks
                self.detectedSplines = splines
                var merged = parsedResponse
                if self.scanMode == .stillPhoto, let relief = self.mountReliefMap {
                    merged = self.analysisResultWithMountRelief(parsedResponse, relief: relief, intrinsics: intrinsics)
                }
                self.analysisResult = merged
                self.usedNativeEngine = usedNative
                self.overlaySource = source
                self.isAnalyzing = false
                self.isCapturing = false
                self.showResultViewer = false
                self.lastCountdownDigit = 0
                self.suppressAutoCaptureUntil = CACurrentMediaTime() + 3.0
                self.guidanceEngine.reset()
                self.cameraSession.start()
                self.successHaptic.notificationOccurred(.success)
            }
        }
    }

    /**
     * Landmark-anchored guides — **fallback only** when CurvEngine XCFramework is missing
     * or native JSON has no `points_image_norm`. Must not ship as the product detection path.
     * Phase 1: always returns *something* so overlay never empty for debugging.
     */
    private func fallbackLandmarkCreases(landmarks: [CGPoint]) -> [CreaseSpline] {
        // If no landmarks yet (e.g. simulator without camera), return centered scaffold so overlay is still testable.
        if landmarks.count < 21 {
            let cx: CGFloat = 0.5, cy: CGFloat = 0.55
            func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: cx + x*0.28, y: cy + y*0.28) }
            return [
                CreaseSpline(id: "heart_line", lineType: .heart, points: [p(-0.6, -0.22), p(-0.1, -0.24), p(0.45, -0.20), p(0.62, -0.12)], confidence: 0.2, tortuosity: 1.1, arcLengthPx: 120),
                CreaseSpline(id: "head_line",  lineType: .head,  points: [p(-0.55, 0.02), p(-0.05, 0.04), p(0.42, 0.06), p(0.62, 0.08)], confidence: 0.2, tortuosity: 1.1, arcLengthPx: 130),
                CreaseSpline(id: "life_line",  lineType: .life,  points: [p(-0.38, -0.02), p(-0.48, 0.18), p(-0.42, 0.38), p(-0.20, 0.58)], confidence: 0.2, tortuosity: 1.2, arcLengthPx: 180),
                CreaseSpline(id: "fate_line",  lineType: .fate,  points: [p(0.02, 0.52), p(0.04, 0.22), p(0.06, -0.02), p(0.06, -0.18)], confidence: 0.2, tortuosity: 1.0, arcLengthPx: 110)
            ]
        }

        let wrist = landmarks[0]
        let thumbCmc = landmarks[1]
        let thumbMcp = landmarks[2]
        let thumbIp = landmarks[3]
        let indexMcp = landmarks[5]
        let middleMcp = landmarks[9]
        let middleTip = landmarks[12]
        let ringMcp = landmarks[13]
        let pinkyMcp = landmarks[17]

        func lerp(_ a: CGPoint, _ b: CGPoint, _ t: CGFloat) -> CGPoint {
            CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t)
        }

        func palmDepth(_ mcp: CGPoint, _ t: CGFloat) -> CGPoint {
            lerp(mcp, wrist, t)
        }

        func length(_ pts: [CGPoint]) -> Float {
            var sum: Float = 0
            for i in 1..<pts.count {
                let dx = Float(pts[i].x - pts[i - 1].x)
                let dy = Float(pts[i].y - pts[i - 1].y)
                sum += sqrt(dx * dx + dy * dy)
            }
            return sum * 1400
        }

        let heart: [CGPoint] = [
            palmDepth(pinkyMcp, 0.06),
            palmDepth(ringMcp, 0.10),
            palmDepth(middleMcp, 0.12),
            palmDepth(indexMcp, 0.10),
            lerp(palmDepth(indexMcp, 0.08), palmDepth(middleMcp, 0.08), 0.35)
        ]
        let lifeOrigin = lerp(indexMcp, thumbMcp, 0.45)
        let head: [CGPoint] = [
            lerp(lifeOrigin, wrist, 0.18),
            palmDepth(indexMcp, 0.32),
            palmDepth(middleMcp, 0.36),
            palmDepth(ringMcp, 0.34),
            palmDepth(pinkyMcp, 0.30)
        ]
        let life: [CGPoint] = [
            lerp(lifeOrigin, wrist, 0.08),
            lerp(thumbMcp, indexMcp, 0.25),
            lerp(thumbMcp, wrist, 0.35),
            lerp(thumbIp, wrist, 0.55),
            lerp(thumbCmc, wrist, 0.72),
            lerp(wrist, thumbCmc, 0.15)
        ]
        let fate: [CGPoint] = [
            lerp(wrist, middleTip, 0.12),
            lerp(wrist, middleTip, 0.38),
            lerp(wrist, middleTip, 0.58),
            palmDepth(middleMcp, 0.22)
        ]

        return [
            CreaseSpline(id: "heart_line", lineType: .heart, points: heart, confidence: 0.86, tortuosity: 1.12, arcLengthPx: length(heart)),
            CreaseSpline(id: "head_line", lineType: .head, points: head, confidence: 0.90, tortuosity: 1.08, arcLengthPx: length(head)),
            CreaseSpline(id: "life_line", lineType: .life, points: life, confidence: 0.93, tortuosity: 1.25, arcLengthPx: length(life)),
            CreaseSpline(id: "fate_line", lineType: .fate, points: fate, confidence: 0.78, tortuosity: 1.03, arcLengthPx: length(fate))
        ]
    }

    /// Bake an upright UIImage into a BGRA CVPixelBuffer for CurvEngine (matches overlay coords).
    static func makeBGRAPixelBuffer(from image: UIImage?) -> CVPixelBuffer? {
        guard let cgImage = image?.cgImage else { return nil }
        let width = cgImage.width
        let height = cgImage.height
        var buffer: CVPixelBuffer?
        let attrs: [CFString: Any] = [
            kCVPixelBufferCGImageCompatibilityKey: true,
            kCVPixelBufferCGBitmapContextCompatibilityKey: true
        ]
        let status = CVPixelBufferCreate(
            kCFAllocatorDefault,
            width,
            height,
            kCVPixelFormatType_32BGRA,
            attrs as CFDictionary,
            &buffer
        )
        guard status == kCVReturnSuccess, let pixelBuffer = buffer else { return nil }

        CVPixelBufferLockBaseAddress(pixelBuffer, [])
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, []) }
        guard let ctx = CGContext(
            data: CVPixelBufferGetBaseAddress(pixelBuffer),
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: CVPixelBufferGetBytesPerRow(pixelBuffer),
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        ) else {
            return nil
        }
        ctx.draw(cgImage, in: CGRect(x: 0, y: 0, width: width, height: height))
        return pixelBuffer
    }

    static let sharedCIContext = CIContext(options: [
        .useSoftwareRenderer: false,
        .priorityRequestLow: false
    ])

    /// Returns an upright UIImage (pixels already rotated) plus the Vision orientation
    /// that matches the raw capture buffer before baking.
    static func uprightImage(
        from pixelBuffer: CVPixelBuffer
    ) -> (UIImage?, CGImagePropertyOrientation) {
        let ciImage = CIImage(cvPixelBuffer: pixelBuffer)
        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)

        // High-res stills are often sensor-landscape even when the connection is portrait.
        if width > height {
            let rotated = ciImage.oriented(.right)
            guard let cgImage = sharedCIContext.createCGImage(rotated, from: rotated.extent) else {
                return (nil, .right)
            }
            return (UIImage(cgImage: cgImage, scale: 1.0, orientation: .up), .right)
        }

        guard let cgImage = sharedCIContext.createCGImage(ciImage, from: ciImage.extent) else {
            return (nil, .up)
        }
        return (UIImage(cgImage: cgImage, scale: 1.0, orientation: .up), .up)
    }
}
