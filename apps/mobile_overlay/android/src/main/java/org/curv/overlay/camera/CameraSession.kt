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

    /** When false, incoming analysis frames are dropped (battery save during review/reading). */
    @Volatile
    var frameAnalysisEnabled: Boolean = true

    private var boundLifecycleOwner: LifecycleOwner? = null
    private var boundSurfaceProvider: Preview.SurfaceProvider? = null
    @Volatile
    private var paused: Boolean = false

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
                cameraProvider?.bindToLifecycle(
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
        onSuccess: (ByteBuffer, Int, Int, Int) -> Unit,
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
                        onSuccess(directBuf, image.width, image.height, image.planes[0].rowStride)
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
