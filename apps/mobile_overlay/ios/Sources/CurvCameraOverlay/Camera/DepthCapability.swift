import AVFoundation
import Foundation

/**
 * Runtime probe for hardware depth capture on the back camera.
 *
 * Depth is strictly opportunistic: every field gates a progressive
 * enhancement, and the orbit pipeline falls back to the parallax/metric
 * estimator (then the tilt proxy) whenever depth is unavailable. This keeps
 * the feature working on all devices while high-end hardware (dual-camera
 * disparity, LiDAR absolute depth) contributes measured millimeters.
 *
 * Cost model is deliberately light: depth is captured only on orbit
 * keyframe stills (<= 6 per scan), downsampled at conversion time, and never
 * streamed. A continuous `AVCaptureDepthDataOutput` preview tap is a
 * possible future step, not this one.
 */
public struct DepthCapability: Sendable, Equatable {
    /// `AVCapturePhotoOutput.isDepthDataDeliverySupported` for the session.
    public var photoDepthSupported: Bool = false
    /// `AVCapturePhotoOutput.isCameraCalibrationDataDeliverySupported`.
    /// Needed for disparity -> meters and for true intrinsics.
    public var calibrationDataSupported: Bool = false
    /// A back LiDAR depth camera exists (absolute time-of-flight meters).
    public var lidarPresent: Bool = false
    /// Any back dual/triple camera exists (stereo disparity depth).
    public var stereoPresent: Bool = false

    public var supported: Bool { photoDepthSupported }

    /// Absolute time-of-flight meters (LiDAR). Otherwise stereo disparity
    /// that must be converted with calibration data.
    public var absoluteDepth: Bool { lidarPresent }

    /// Per-frame diagnostic source string written into `OrbitFrame.depthSource`.
    public var depthSource: String {
        lidarPresent ? "lidar_avdepth_v1" : "disparity_avdepth_v1"
    }

    /// Probe after the photo output is added to a running session config.
    /// Safe on Simulator (all false) and on single-camera devices.
    public static func probe(photoOutput: AVCapturePhotoOutput?) -> DepthCapability {
        var cap = DepthCapability()
        guard let photoOutput else { return cap }
        cap.photoDepthSupported = photoOutput.isDepthDataDeliverySupported
        cap.calibrationDataSupported = photoOutput.isCameraCalibrationDataDeliverySupported
        let discovery = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInLiDARDepthCamera, .builtInDualCamera,
                          .builtInDualWideCamera, .builtInTripleCamera,
                          .builtInTrueDepthCamera],
            mediaType: .video,
            position: .back
        )
        for device in discovery.devices {
            switch device.deviceType {
            case .builtInLiDARDepthCamera:
                cap.lidarPresent = true
            case .builtInDualCamera, .builtInDualWideCamera, .builtInTripleCamera:
                cap.stereoPresent = true
            default:
                break
            }
        }
        return cap
    }
}
