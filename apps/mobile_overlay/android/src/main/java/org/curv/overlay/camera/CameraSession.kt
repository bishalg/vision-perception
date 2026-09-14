package org.curv.overlay.camera

import android.content.Context
import android.util.Log
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleOwner
import java.nio.ByteBuffer
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * Shared CameraX session: preview + 30+ FPS analysis + high-res stills.
 */
class CameraSession(private val context: Context) {
    private val tag = "OverlayCameraSession"

    private var cameraProvider: ProcessCameraProvider? = null
    private var imageAnalysis: ImageAnalysis? = null
    private var imageCapture: ImageCapture? = null
    private val analysisExecutor: ExecutorService = Executors.newSingleThreadExecutor()

    var onFrameAnalysis: ((ImageProxy) -> Unit)? = null

    /**
     * Hardware depth probe (Camera2 DEPTH_OUTPUT capability). False on most
     * devices; the orbit pipeline treats that as "use the fallback path".
     * Captured CameraX stills carry no depth yet — see [DepthCapability].
     */
    val depthCapability: DepthCapability by lazy {
        val cap = DepthCapability.probe(context)
        Log.i(tag, "Depth capability: supported=${cap.supported}")
        cap
    }

    /** When false, incoming analysis frames are dropped (battery save during review/reading). */
    @Volatile
    var frameAnalysisEnabled: Boolean = true

    private var boundLifecycleOwner: LifecycleOwner? = null
    private var boundSurfaceProvider: Preview.SurfaceProvider? = null
    private var activeCamera: androidx.camera.core.Camera? = null
    @Volatile
    private var paused: Boolean = false

    fun setMacroFocus(enabled: Boolean) {
        val camera = activeCamera ?: return
        val zoomRatio = if (enabled) 2.0f else 1.0f
        camera.cameraControl.setZoomRatio(zoomRatio)
    }

    fun startCamera(
        lifecycleOwner: LifecycleOwner,
        surfaceProvider: Preview.SurfaceProvider,
        onSuccess: () -> Unit = {},
        onError: (Exception) -> Unit = {}
    ) {
        boundLifecycleOwner = lifecycleOwner
        boundSurfaceProvider = surfaceProvider
        paused = false
        frameAnalysisEnabled = true
        val cameraProviderFuture = ProcessCameraProvider.getInstance(context)
        cameraProviderFuture.addListener({
            try {
                cameraProvider = cameraProviderFuture.get()

                val preview = Preview.Builder().build().also {
                    it.setSurfaceProvider(surfaceProvider)
                }

                imageAnalysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .setOutputImageFormat(ImageAnalysis.OUTPUT_IMAGE_FORMAT_YUV_420_888)
                    .build()
                    .also { analysis ->
                        analysis.setAnalyzer(analysisExecutor) { imageProxy ->
                            if (frameAnalysisEnabled && !paused) {
                                onFrameAnalysis?.invoke(imageProxy)
                            } else {
                                imageProxy.close()
                            }
                        }
                    }

                imageCapture = ImageCapture.Builder()
                    .setCaptureMode(ImageCapture.CAPTURE_MODE_MAXIMIZE_QUALITY)
                    .build()

                cameraProvider?.unbindAll()
                activeCamera = cameraProvider?.bindToLifecycle(
                    lifecycleOwner,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    preview,
                    imageAnalysis,
                    imageCapture
                )
                onSuccess()
            } catch (exc: Exception) {
                Log.e(tag, "CameraX binding failed", exc)
                onError(exc)
            }
        }, ContextCompat.getMainExecutor(context))
    }

    fun capturePhoto(
        onSuccess: (CapturedPhoto) -> Unit,
        onError: (Exception) -> Unit
    ) {
        val capture = imageCapture ?: run {
            onError(IllegalStateException("ImageCapture not initialized"))
            return
        }

        capture.takePicture(
            analysisExecutor,
            object : ImageCapture.OnImageCapturedCallback() {
                override fun onCaptureSuccess(image: ImageProxy) {
                    try {
                        val buffer = image.planes[0].buffer
                        val directBuf = ByteBuffer.allocateDirect(buffer.remaining())
                        directBuf.put(buffer)
                        directBuf.flip()
                        val intrinsics = CameraIntrinsics.fromFieldOfView(
                            imageWidth = image.width,
                            imageHeight = image.height,
                            fovHorizontalDeg = 68f
                        )
                        onSuccess(
                            CapturedPhoto(
                                buffer = directBuf,
                                width = image.width,
                                height = image.height,
                                stride = image.planes[0].rowStride,
                                intrinsics = intrinsics
                            )
                        )
                    } finally {
                        image.close()
                    }
                }

                override fun onError(exception: ImageCaptureException) {
                    Log.e(tag, "Image capture failed", exception)
                    onError(exception)
                }
            }
        )
    }

    /**
     * Samples per-mount mean depth in mm from an available [PhotoDepthMap]
     * against normalized palm landmarks. Returns null when depth is unsupported,
     * invalid, or below quality thresholds (caller falls back to metric parallax).
     */
    fun sampleMountDepth(
        depthMap: PhotoDepthMap?,
        landmarks: List<List<Float>>
    ): Map<String, Float>? {
        if (depthMap == null || !depthCapability.supported) return null
        return org.curv.overlay.palm.MountDepthSampler.sampleMountMeansMm(depthMap, landmarks)
    }

    fun shutdown() {
        analysisExecutor.shutdown()
        cameraProvider?.unbindAll()
    }

    /**
     * Battery-save pause during review / reading — parity with iOS
     * `cameraSession.stop()`. Drops analysis frames; preview keeps last frame.
     */
    fun pause() {
        paused = true
        frameAnalysisEnabled = false
    }

    /** Resume live guidance — parity with iOS `cameraSession.start()`. */
    fun resume() {
        paused = false
        frameAnalysisEnabled = true
    }

    /** Full stop (unbind). Call [startCamera] again to restart. */
    fun stop() {
        paused = true
        frameAnalysisEnabled = false
        try {
            cameraProvider?.unbindAll()
        } catch (_: Exception) { }
    }

    /** Restart after [stop] using the last bound lifecycle + surface. */
    fun start() {
        val owner = boundLifecycleOwner
        val provider = boundSurfaceProvider
        if (owner != null && provider != null) {
            startCamera(lifecycleOwner = owner, surfaceProvider = provider)
        } else {
            paused = false
            frameAnalysisEnabled = true
        }
    }
}
