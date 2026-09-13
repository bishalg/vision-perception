import Foundation
import CoreVideo
import Darwin

#if canImport(curv_capi)
import curv_capi
#endif

public enum CurvError: Error {
    case initializationFailed
    case invalidBuffer
    case frameCreationFailed
    case pipelineExecutionFailed(Int)
    case bufferTooSmall
    /// Native `libcurv_capi` / XCFramework is not linked in this build.
    case nativeUnavailable
}

public enum CurvPixelFormat: Int32 {
    case grayscale = 1
    case rgb = 2
    case rgba = 3
    case bgr = 4
    case bgra = 5
}

/**
 * Swift wrapper for CurvEngine native perception core.
 *
 * When `CurvEngine.xcframework` (module `curv_capi`) is linked, calls go to the
 * C-ABI. Without it, init throws `.nativeUnavailable` so the camera UI can still
 * run for overlay / guidance validation on device.
 */
public final class CurvEngine {

    public static var isNativeAvailable: Bool {
        #if canImport(curv_capi)
        return true
        #else
        return false
        #endif
    }

    #if canImport(curv_capi)
    private var pipelineHandle: OpaquePointer?
    #endif

    public init(domainId: String = "palm") throws {
        #if canImport(curv_capi)
        let handle = domainId.withCString { curv_pipeline_create($0) }
        guard let validHandle = handle else {
            throw CurvError.initializationFailed
        }
        self.pipelineHandle = OpaquePointer(validHandle)
        #else
        throw CurvError.nativeUnavailable
        #endif
    }

    deinit {
        #if canImport(curv_capi)
        if let handle = pipelineHandle {
            curv_pipeline_destroy(UnsafeMutableRawPointer(handle))
            pipelineHandle = nil
        }
        #endif
    }

    public func processLandmarksRoi(
        bytes: UnsafePointer<UInt8>,
        width: Int,
        height: Int,
        stride: Int = 0,
        format: CurvPixelFormat = .bgra,
        landmarks: [Float]
    ) -> Result<String, CurvError> {
        #if canImport(curv_capi)
        guard let handle = pipelineHandle else {
            return .failure(.initializationFailed)
        }
        guard landmarks.count >= 42 else {
            return .failure(.invalidBuffer)
        }

        guard let frameHandle = curv_frame_create_from_buffer(
            bytes,
            Int32(width),
            Int32(height),
            Int32(stride),
            format.rawValue
        ) else {
            return .failure(.frameCreationFailed)
        }

        defer { curv_frame_destroy(frameHandle) }

        var jsonBuffer = [CChar](repeating: 0, count: 2097152)
        var status = landmarks.withUnsafeBufferPointer { lmsPtr in
            curv_pipeline_process_landmarks_roi(
                UnsafeMutableRawPointer(handle),
                frameHandle,
                lmsPtr.baseAddress,
                &jsonBuffer,
                Int32(jsonBuffer.count)
            )
        }

        if status == 4 { // CURV_STATUS_BUFFER_TOO_SMALL
            var neededSize: Int32 = 0
            curv_pipeline_get_last_json(UnsafeMutableRawPointer(handle), nil, 0, &neededSize)
            if neededSize > 0 {
                jsonBuffer = [CChar](repeating: 0, count: Int(neededSize))
                status = curv_pipeline_get_last_json(
                    UnsafeMutableRawPointer(handle),
                    &jsonBuffer,
                    Int32(jsonBuffer.count),
                    nil
                )
            }
        }

        if status != 0 && status != 2 {
            return .failure(.pipelineExecutionFailed(Int(status)))
        }
        return .success(String(cString: jsonBuffer))
        #else
        return .failure(.nativeUnavailable)
        #endif
    }

    public func processQuadRoi(
        bytes: UnsafePointer<UInt8>,
        width: Int,
        height: Int,
        stride: Int = 0,
        format: CurvPixelFormat = .bgra,
        corners: [Float]
    ) -> Result<String, CurvError> {
        #if canImport(curv_capi)
        guard let handle = pipelineHandle else {
            return .failure(.initializationFailed)
        }
        guard corners.count >= 8 else {
            return .failure(.invalidBuffer)
        }

        guard let frameHandle = curv_frame_create_from_buffer(
            bytes,
            Int32(width),
            Int32(height),
            Int32(stride),
            format.rawValue
        ) else {
            return .failure(.frameCreationFailed)
        }

        defer { curv_frame_destroy(frameHandle) }

        var jsonBuffer = [CChar](repeating: 0, count: 2097152)
        var status = corners.withUnsafeBufferPointer { cornersPtr in
            curv_pipeline_process_quad_roi(
                UnsafeMutableRawPointer(handle),
                frameHandle,
                cornersPtr.baseAddress,
                &jsonBuffer,
                Int32(jsonBuffer.count)
            )
        }

        if status == 4 {
            var neededSize: Int32 = 0
            curv_pipeline_get_last_json(UnsafeMutableRawPointer(handle), nil, 0, &neededSize)
            if neededSize > 0 {
                jsonBuffer = [CChar](repeating: 0, count: Int(neededSize))
                status = curv_pipeline_get_last_json(
                    UnsafeMutableRawPointer(handle),
                    &jsonBuffer,
                    Int32(jsonBuffer.count),
                    nil
                )
            }
        }

        if status != 0 && status != 2 {
            return .failure(.pipelineExecutionFailed(Int(status)))
        }
        return .success(String(cString: jsonBuffer))
        #else
        return .failure(.nativeUnavailable)
        #endif
    }

    public func processPixelBuffer(
        pixelBuffer: CVPixelBuffer,
        landmarks: [Float],
        includeMinorRidges: Bool = false
    ) -> Result<String, CurvError> {
        withLockedPixelBuffer(pixelBuffer) { bytes, width, height, stride in
            if includeMinorRidges {
                return processLandmarksRoiExtended(
                    bytes: bytes,
                    width: width,
                    height: height,
                    stride: stride,
                    format: .bgra,
                    landmarks: landmarks,
                    includeMinorRidges: true
                )
            }
            return processLandmarksRoi(
                bytes: bytes,
                width: width,
                height: height,
                stride: stride,
                format: .bgra,
                landmarks: landmarks
            )
        }
    }

    /// Orbit mount-relief analysis. Uses native C-API when the linked XCFramework
    /// contains `curv_analyze_orbit_session_json`; otherwise returns a passthrough
    /// envelope so Swift-side `MountReliefEstimator` remains the source of truth.
    public func analyzeOrbitSession(json: String) -> Result<String, CurvError> {
        #if canImport(curv_capi)
        if let analyze = Self.dlsymAnalyzeOrbit {
            var jsonBuffer = [CChar](repeating: 0, count: 65536)
            let status = json.withCString { ptr in
                analyze(ptr, &jsonBuffer, Int32(jsonBuffer.count))
            }
            if status == 0 {
                return .success(String(cString: jsonBuffer))
            }
        }
        #endif
        // Fallback: app already grades mounts in Swift; keep schema compatible.
        _ = json
        return .success(
            #"{"domain_payload":{"mount_relief":{"source":"swift_fallback","venus_elevated":false,"mounts":[]}}}"#
        )
    }

    /// Fingertip dermatoglyphics. Prefers native classifier when present in the
    /// linked binary; otherwise returns an `unknown` pattern stub (no template).
    public func classifyFingertip(
        pixelBuffer: CVPixelBuffer,
        landmarks: [Float],
        digit: String
    ) -> Result<String, CurvError> {
        #if canImport(curv_capi)
        if let classify = Self.dlsymClassifyFingertip {
            return withLockedPixelBuffer(pixelBuffer) { bytes, width, height, stride in
                guard let handle = pipelineHandle else { return .failure(.initializationFailed) }
                guard landmarks.count >= 42 else { return .failure(.invalidBuffer) }
                guard let frameHandle = curv_frame_create_from_buffer(
                    bytes, Int32(width), Int32(height), Int32(stride), CurvPixelFormat.bgra.rawValue
                ) else { return .failure(.frameCreationFailed) }
                defer { curv_frame_destroy(frameHandle) }
                var jsonBuffer = [CChar](repeating: 0, count: 65536)
                let status = landmarks.withUnsafeBufferPointer { lmsPtr in
                    digit.withCString { digitPtr in
                        classify(
                            UnsafeMutableRawPointer(handle),
                            frameHandle,
                            lmsPtr.baseAddress,
                            digitPtr,
                            &jsonBuffer,
                            Int32(jsonBuffer.count)
                        )
                    }
                }
                if status != 0 { return .failure(.pipelineExecutionFailed(Int(status))) }
                return .success(String(cString: jsonBuffer))
            }
        }
        #endif
        let escaped = digit.replacingOccurrences(of: "\"", with: "")
        return .success(
            "{\"domain_payload\":{\"fingertip_pattern\":{\"digit\":\"\(escaped)\",\"pattern\":\"unknown\",\"ridge_frequency\":0,\"quality\":0}}}"
        )
    }

    /// Extended palm ROI. Falls back to standard landmarks ROI when the linked
    /// XCFramework predates `curv_pipeline_process_landmarks_roi_extended`.
    public func processLandmarksRoiExtended(
        bytes: UnsafePointer<UInt8>,
        width: Int,
        height: Int,
        stride: Int = 0,
        format: CurvPixelFormat = .bgra,
        landmarks: [Float],
        includeMinorRidges: Bool
    ) -> Result<String, CurvError> {
        #if canImport(curv_capi)
        if includeMinorRidges, let extended = Self.dlsymProcessExtended {
            guard let handle = pipelineHandle else { return .failure(.initializationFailed) }
            guard landmarks.count >= 42 else { return .failure(.invalidBuffer) }
            guard let frameHandle = curv_frame_create_from_buffer(
                bytes, Int32(width), Int32(height), Int32(stride), format.rawValue
            ) else { return .failure(.frameCreationFailed) }
            defer { curv_frame_destroy(frameHandle) }
            var jsonBuffer = [CChar](repeating: 0, count: 2097152)
            let status = landmarks.withUnsafeBufferPointer { lmsPtr in
                extended(
                    UnsafeMutableRawPointer(handle),
                    frameHandle,
                    lmsPtr.baseAddress,
                    1,
                    &jsonBuffer,
                    Int32(jsonBuffer.count)
                )
            }
            if status == 0 || status == 2 {
                return .success(String(cString: jsonBuffer))
            }
        }
        #endif
        return processLandmarksRoi(
            bytes: bytes,
            width: width,
            height: height,
            stride: stride,
            format: format,
            landmarks: landmarks
        )
    }

    // MARK: - Optional 3D-scan C-API (resolved at runtime)

    // The shipped CurvEngine.xcframework may predate these symbols. Resolve via
    // dlsym so Swift compiles against old module headers and picks them up after
    // `scripts/build_ios_xcframework.sh` without a source change.
    #if canImport(curv_capi)
    private typealias AnalyzeOrbitFn = @convention(c) (
        UnsafePointer<CChar>?, UnsafeMutablePointer<CChar>?, Int32
    ) -> Int32
    private typealias ProcessExtendedFn = @convention(c) (
        UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<Float>?, Int32,
        UnsafeMutablePointer<CChar>?, Int32
    ) -> Int32
    private typealias ClassifyFingertipFn = @convention(c) (
        UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<Float>?,
        UnsafePointer<CChar>?, UnsafeMutablePointer<CChar>?, Int32
    ) -> Int32

    private static let dlsymAnalyzeOrbit: AnalyzeOrbitFn? = {
        // RTLD_DEFAULT is a C macro ((void*)-2); not always imported into Swift.
        let rtldDefault = UnsafeMutableRawPointer(bitPattern: -2)
        guard let sym = dlsym(rtldDefault, "curv_analyze_orbit_session_json") else { return nil }
        return unsafeBitCast(sym, to: AnalyzeOrbitFn.self)
    }()

    private static let dlsymProcessExtended: ProcessExtendedFn? = {
        let rtldDefault = UnsafeMutableRawPointer(bitPattern: -2)
        guard let sym = dlsym(rtldDefault, "curv_pipeline_process_landmarks_roi_extended") else {
            return nil
        }
        return unsafeBitCast(sym, to: ProcessExtendedFn.self)
    }()

    private static let dlsymClassifyFingertip: ClassifyFingertipFn? = {
        let rtldDefault = UnsafeMutableRawPointer(bitPattern: -2)
        guard let sym = dlsym(rtldDefault, "curv_classify_fingertip_roi") else { return nil }
        return unsafeBitCast(sym, to: ClassifyFingertipFn.self)
    }()
    #endif

    public func processPixelBuffer(
        pixelBuffer: CVPixelBuffer,
        corners: [Float]
    ) -> Result<String, CurvError> {
        withLockedPixelBuffer(pixelBuffer) { bytes, width, height, stride in
            processQuadRoi(
                bytes: bytes,
                width: width,
                height: height,
                stride: stride,
                format: .bgra,
                corners: corners
            )
        }
    }

    private func withLockedPixelBuffer(
        _ pixelBuffer: CVPixelBuffer,
        body: (UnsafePointer<UInt8>, Int, Int, Int) -> Result<String, CurvError>
    ) -> Result<String, CurvError> {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            return .failure(.invalidBuffer)
        }

        return body(
            baseAddress.assumingMemoryBound(to: UInt8.self),
            CVPixelBufferGetWidth(pixelBuffer),
            CVPixelBufferGetHeight(pixelBuffer),
            CVPixelBufferGetBytesPerRow(pixelBuffer)
        )
    }
}
