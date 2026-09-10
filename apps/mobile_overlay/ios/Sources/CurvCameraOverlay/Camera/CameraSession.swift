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
    private var photoCaptureCompletion: ((Result<CVPixelBuffer, Error>) -> Void)?

    public override init() {
        super.init()
        configureSession()
    }

    private func configureSession() {
        sessionQueue.async { [weak self] in
            guard let self else { return }
            self.captureSession.beginConfiguration()
            self.captureSession.sessionPreset = .hd1920x1080

            guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back),
                  let input = try? AVCaptureDeviceInput(device: device) else {
                self.logger.error("Failed to acquire back camera device")
                self.captureSession.commitConfiguration()
                return
            }

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
                    self.photoOutput.maxPhotoDimensions = CMVideoDimensions(width: 4032, height: 3024)
                } else {
                    // On iOS versions prior to 16, high resolution capture is the default when supported.
                    // The `isHighResolutionCaptureEnabled` property is deprecated in iOS 16+, so we avoid using it.
                }
                self.applyPortraitOrientation(to: self.photoOutput)
            }

            self.captureSession.commitConfiguration()
        }
    }

    private func applyPortraitOrientation(to output: AVCaptureOutput) {
        guard let connection = output.connection(with: .video), connection.isVideoOrientationSupported else {
            return
        }
        connection.videoOrientation = .portrait
    }

    public func start() {
        sessionQueue.async { [weak self] in
            guard let self, !self.captureSession.isRunning else { return }
            self.captureSession.startRunning()
        }
    }

    public func stop() {
        sessionQueue.async { [weak self] in
            guard let self, self.captureSession.isRunning else { return }
            self.captureSession.stopRunning()
        }
    }

    public func capturePhoto(completion: @escaping (Result<CVPixelBuffer, Error>) -> Void) {
        sessionQueue.async { [weak self] in
            guard let self else { return }
            // Keep stills in the same portrait orientation as the live preview.
            self.applyPortraitOrientation(to: self.photoOutput)
            self.photoCaptureCompletion = completion
            let settings = AVCapturePhotoSettings(format: [
                kCVPixelBufferPixelFormatTypeKey as String: Int(kCVPixelFormatType_32BGRA)
            ])
            if #available(iOS 16.0, *) {
                settings.maxPhotoDimensions = CMVideoDimensions(width: 4032, height: 3024)
            } else {
                // Pre–iOS 16 fallback: enable high-resolution capture when supported.
                // This API is deprecated starting in iOS 16.
                settings.isHighResolutionPhotoEnabled = true
            }
            self.photoOutput.capturePhoto(with: settings, delegate: self)
        }
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
        photoCaptureCompletion?(.success(pixelBuffer))
        photoCaptureCompletion = nil
    }
}
