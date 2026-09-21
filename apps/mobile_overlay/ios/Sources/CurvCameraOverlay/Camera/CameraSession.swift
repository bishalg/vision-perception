import AVFoundation
import CoreVideo
import Foundation
import os.log

/**
 * Shared AVFoundation camera session: 60 FPS preview frames + high-res stills.
 * Overlay apps do not own their own capture pipeline.
 */
public final class CameraSession: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, AVCapturePhotoCaptureDelegate {
    private let logger = Logger(subsystem: "org.curv.overlay", category: "CameraSession")

    public let captureSession = AVCaptureSession()
    private let videoOutput = AVCaptureVideoDataOutput()
    private let photoOutput = AVCapturePhotoOutput()
    private let sessionQueue = DispatchQueue(label: "org.curv.overlay.sessionQueue")
    private let videoQueue = DispatchQueue(label: "org.curv.overlay.videoQueue", qos: .userInteractive)

    public var onFrameReceived: ((CVPixelBuffer) -> Void)?
    private var photoCaptureCompletion: ((Result<CapturedPhoto, Error>) -> Void)?
    private var pendingDepthSource: String?
    private weak var activeCaptureDevice: AVCaptureDevice?

    private var isConfigured: Bool = false
    /// Hardware depth probe result. All-false on Simulator / single-camera
    /// devices; every consumer must treat that as "use the fallback path".
    public private(set) var depthCapability = DepthCapability()

    public override init() {
        super.init()
        checkPermissionsAndConfigure()
    }

    private func checkPermissionsAndConfigure() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureSession()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                guard let self else { return }
                if granted {
                    self.configureSession()
                    self.start()
                } else {
                    self.logger.error("Camera access denied by user")
                }
            }
        default:
            logger.error("Camera authorization unavailable or denied")
        }
    }

    private func configureSession() {
        sessionQueue.async { [weak self] in
            guard let self, !self.isConfigured else { return }
            self.captureSession.beginConfiguration()
            self.captureSession.sessionPreset = .hd1920x1080

            let discovery = AVCaptureDevice.DiscoverySession(
                deviceTypes: [
                    .builtInLiDARDepthCamera,
                    .builtInTripleCamera,
                    .builtInDualWideCamera,
                    .builtInDualCamera,
                    .builtInWideAngleCamera
                ],
                mediaType: .video,
                position: .back
            )
            guard let device = discovery.devices.first ?? AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back),
                  let input = try? AVCaptureDeviceInput(device: device) else {
                self.logger.error("Failed to acquire back camera device")
                self.captureSession.commitConfiguration()
                return
            }
            self.activeCaptureDevice = device

            if self.captureSession.canAddInput(input) {
                self.captureSession.addInput(input)
            }

            self.videoOutput.alwaysDiscardsLateVideoFrames = true
            self.videoOutput.videoSettings = [
                kCVPixelBufferPixelFormatTypeKey as String: Int(kCVPixelFormatType_32BGRA)
            ]
            self.videoOutput.setSampleBufferDelegate(self, queue: self.videoQueue)

            if self.captureSession.canAddOutput(self.videoOutput) {
                self.captureSession.addOutput(self.videoOutput)
                if let connection = self.videoOutput.connection(with: .video) {
                    connection.videoOrientation = .portrait
                }
            }

            if self.captureSession.canAddOutput(self.photoOutput) {
                self.captureSession.addOutput(self.photoOutput)
                if #available(iOS 16.0, *) {
                    // Prefer highest supported still resolution for close-up / orbit keyframes.
                    let supported = device.activeFormat.supportedMaxPhotoDimensions
                    if let best = supported.max(by: { a, b in
                        Int64(a.width) * Int64(a.height) < Int64(b.width) * Int64(b.height)
                    }) {
                        self.photoOutput.maxPhotoDimensions = best
                        self.logger.info("Photo max dimensions: \(best.width)x\(best.height)")
                    }
                } else {
                    self.photoOutput.isHighResolutionCaptureEnabled = true
                }
                self.applyPortraitOrientation(to: self.photoOutput)
            }

            self.captureSession.commitConfiguration()
            self.isConfigured = true
            self.depthCapability = DepthCapability.probe(photoOutput: self.photoOutput)
            self.logger.info("CameraSession configured successfully (1080p stream + photo)")
        }
    }

    private func applyPortraitOrientation(to output: AVCaptureOutput) {
        guard let connection = output.connection(with: .video), connection.isVideoOrientationSupported else {
            return
        }
        connection.videoOrientation = .portrait
    }

    public func start() {
        if !isConfigured {
            checkPermissionsAndConfigure()
        }
        sessionQueue.async { [weak self] in
            guard let self, self.isConfigured, !self.captureSession.isRunning else { return }
            self.captureSession.startRunning()
        }
    }

    public func stop() {
        sessionQueue.async { [weak self] in
            guard let self, self.captureSession.isRunning else { return }
            self.captureSession.stopRunning()
        }
    }

    /// Configures close-up / macro auto-focus parameters for fine crease and fingertip scans.
    public func setMacroFocus(enabled: Bool) {
        sessionQueue.async { [weak self] in
            guard let self, let device = self.activeCaptureDevice else { return }
            do {
                try device.lockForConfiguration()
                if device.isAutoFocusRangeRestrictionSupported {
                    device.autoFocusRangeRestriction = enabled ? .near : .none
                }
                if device.isFocusModeSupported(.continuousAutoFocus) {
                    device.focusMode = .continuousAutoFocus
                }
                if device.isFocusPointOfInterestSupported {
                    device.focusPointOfInterest = CGPoint(x: 0.5, y: 0.5)
                }
                if device.isExposurePointOfInterestSupported {
                    device.exposurePointOfInterest = CGPoint(x: 0.5, y: 0.5)
                }
                if device.isSubjectAreaChangeMonitoringEnabled == false {
                    device.isSubjectAreaChangeMonitoringEnabled = true
                }
                let targetZoom: CGFloat = enabled ? 2.0 : 1.0
                let clampedZoom = min(max(targetZoom, device.minAvailableVideoZoomFactor), min(device.maxAvailableVideoZoomFactor, 3.0))
                device.videoZoomFactor = clampedZoom
                device.unlockForConfiguration()
                self.logger.info("Macro focus restriction: \(enabled ? "near (2.0x zoom)" : "none (1.0x)")")
            } catch {
                self.logger.error("Failed to update macro focus configuration: \(error.localizedDescription)")
            }
        }
    }

    /// Captures a high-res still with pinhole intrinsics metadata.
    public func capturePhoto(completion: @escaping (Result<CapturedPhoto, Error>) -> Void) {
        sessionQueue.async { [weak self] in
            guard let self else { return }
            guard self.captureSession.isRunning else {
                completion(.failure(NSError(domain: "org.curv.overlay", code: -2, userInfo: [NSLocalizedDescriptionKey: "Camera session is not running"])))
                return
            }

            self.applyPortraitOrientation(to: self.photoOutput)
            self.photoCaptureCompletion = completion

            let settings: AVCapturePhotoSettings
            if self.photoOutput.availablePhotoPixelFormatTypes.contains(kCVPixelFormatType_32BGRA) {
                settings = AVCapturePhotoSettings(format: [
                    kCVPixelBufferPixelFormatTypeKey as String: Int(kCVPixelFormatType_32BGRA)
                ])
            } else {
                settings = AVCapturePhotoSettings()
            }

            if #available(iOS 16.0, *) {
                settings.maxPhotoDimensions = self.photoOutput.maxPhotoDimensions
            } else {
                settings.isHighResolutionPhotoEnabled = true
            }
            // Opportunistic hardware depth: strictly guarded, never throws.
            // Unsupported devices keep these off and flow through the fallback.
            if self.depthCapability.photoDepthSupported {
                settings.isDepthDataDeliveryEnabled = true
            }
            if self.depthCapability.calibrationDataSupported {
                settings.isCameraCalibrationDataDeliveryEnabled = true
            }
            self.pendingDepthSource =
                self.depthCapability.supported ? self.depthCapability.depthSource : nil
            self.photoOutput.capturePhoto(with: settings, delegate: self)
        }
    }

    /// Legacy buffer-only capture (intrinsics approximated from device FOV).
    public func capturePhotoBuffer(completion: @escaping (Result<CVPixelBuffer, Error>) -> Void) {
        capturePhoto { result in
            completion(result.map(\.pixelBuffer))
        }
    }

    public func intrinsicsForDevice(imageWidth: Int, imageHeight: Int) -> CameraIntrinsics {
        guard let device = activeCaptureDevice else {
            return CameraIntrinsics.fromFieldOfView(
                imageWidth: imageWidth,
                imageHeight: imageHeight,
                fovHorizontalDeg: 68
            )
        }
        let format = device.activeFormat
        let fov = format.videoFieldOfView
        if fov > 1 {
            return CameraIntrinsics.fromFieldOfView(
                imageWidth: imageWidth,
                imageHeight: imageHeight,
                fovHorizontalDeg: fov
            )
        }
        let dims = CMVideoFormatDescriptionGetDimensions(format.formatDescription)
        let fx = Float(dims.width) * 0.85
        return CameraIntrinsics(
            focalLengthX: fx,
            focalLengthY: fx,
            principalPointX: Float(imageWidth) * 0.5,
            principalPointY: Float(imageHeight) * 0.5,
            imageWidth: imageWidth,
            imageHeight: imageHeight,
            fieldOfViewDeg: fov > 0 ? fov : nil
        )
    }

    public func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        onFrameReceived?(pixelBuffer)
    }

    public func photoOutput(
        _ output: AVCapturePhotoOutput,
        didFinishProcessingPhoto photo: AVCapturePhoto,
        error: Error?
    ) {
        if let error {
            photoCaptureCompletion?(.failure(error))
            photoCaptureCompletion = nil
            return
        }
        guard let pixelBuffer = photo.pixelBuffer else {
            let err = NSError(
                domain: "org.curv.overlay",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "No pixel buffer in captured photo"]
            )
            photoCaptureCompletion?(.failure(err))
            photoCaptureCompletion = nil
            return
        }
        let w = CVPixelBufferGetWidth(pixelBuffer)
        let h = CVPixelBufferGetHeight(pixelBuffer)
        let calibration = photo.cameraCalibrationData
        // True intrinsics from calibration when delivered; FOV approximation
        // otherwise. Either way the pipeline keeps working.
        let intrinsics: CameraIntrinsics
        if let calibration,
           let calibrated = Self.intrinsicsFromCalibration(calibration, imageWidth: w, imageHeight: h) {
            intrinsics = calibrated
        } else {
            intrinsics = intrinsicsForDevice(imageWidth: w, imageHeight: h)
        }
        let depthMap = photo.depthData.flatMap {
            PhotoDepthMap.from(depthData: $0, calibration: calibration,
                               source: self.pendingDepthSource ?? "depth_v1")
        }
        self.pendingDepthSource = nil
        photoCaptureCompletion?(.success(CapturedPhoto(
            pixelBuffer: pixelBuffer, intrinsics: intrinsics, depthMap: depthMap
        )))
        photoCaptureCompletion = nil
    }

    /// Scale the calibration intrinsic matrix (defined for its reference
    /// dimensions) onto the delivered still size.
    private static func intrinsicsFromCalibration(
        _ calibration: AVCameraCalibrationData, imageWidth w: Int, imageHeight h: Int
    ) -> CameraIntrinsics? {
        let ref = calibration.intrinsicMatrixReferenceDimensions
        guard ref.width > 0, ref.height > 0, w > 0, h > 0 else { return nil }
        let sx = Float(w) / Float(ref.width)
        let sy = Float(h) / Float(ref.height)
        let m = calibration.intrinsicMatrix
        guard m.columns.0.x > 0, m.columns.1.y > 0 else { return nil }
        return CameraIntrinsics(
            focalLengthX: m.columns.0.x * sx,
            focalLengthY: m.columns.1.y * sy,
            principalPointX: m.columns.2.x * sx,
            principalPointY: m.columns.2.y * sy,
            imageWidth: w,
            imageHeight: h,
            fieldOfViewDeg: nil
        )
    }
}
