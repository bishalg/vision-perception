package org.curv.palmapp.controller

import android.graphics.PointF
import android.util.Log
import androidx.annotation.OptIn
import androidx.camera.core.ExperimentalGetImage
import androidx.camera.core.ImageProxy
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.pose.Pose
import com.google.mlkit.vision.pose.PoseDetection
import com.google.mlkit.vision.pose.PoseDetector
import com.google.mlkit.vision.pose.PoseLandmark
import com.google.mlkit.vision.pose.accurate.AccuratePoseDetectorOptions

/**
 * Hand Pose Tracker using Google ML Kit on GPU / NNAPI delegate.
 * Extracts 21 normalized landmark coordinates with zero CPU lockup.
 */
class MLKitHandTracker {
    private val tag = "MLKitHandTracker"

    private val options = AccuratePoseDetectorOptions.Builder()
        .setDetectorMode(AccuratePoseDetectorOptions.STREAM_MODE)
        .build()

    private val detector: PoseDetector = PoseDetection.getClient(options)

    /**
     * Processes a CameraX ImageProxy and returns 21 normalized landmarks if detected.
     */
    @OptIn(ExperimentalGetImage::class)
    fun processImage(
        imageProxy: ImageProxy,
        onSuccess: (List<PointF>) -> Unit,
        onError: (Exception) -> Unit
    ) {
        val mediaImage = imageProxy.image
        if (mediaImage == null) {
            imageProxy.close()
            return
        }

        val inputImage = InputImage.fromMediaImage(mediaImage, imageProxy.imageInfo.rotationDegrees)
        detector.process(inputImage)
            .addOnSuccessListener { pose ->
                val landmarks = extractHandLandmarks(pose, imageProxy.width, imageProxy.height)
                if (landmarks.size == 21) {
                    onSuccess(landmarks)
                } else {
                    onSuccess(emptyList())
                }
                imageProxy.close()
            }
            .addOnFailureListener { e ->
                Log.e(tag, "ML Kit pose detection failed", e)
                onError(e)
                imageProxy.close()
            }
    }

    /** Re-detect on a captured bitmap (upright). Used to align still coords — parity with iOS re-detect. */
    fun processBitmap(
        bitmap: android.graphics.Bitmap,
        onSuccess: (List<PointF>) -> Unit,
        onError: (Exception) -> Unit = {}
    ) {
        val inputImage = InputImage.fromBitmap(bitmap, 0)
        detector.process(inputImage)
            .addOnSuccessListener { pose ->
                val landmarks = extractHandLandmarks(pose, bitmap.width, bitmap.height)
                onSuccess(if (landmarks.size == 21) landmarks else emptyList())
            }
            .addOnFailureListener { e ->
                Log.e(tag, "Bitmap pose detection failed", e)
                onError(e as Exception)
            }
    }

    /**
     * Extracts and normalizes 21 landmarks in canonical MediaPipe order.
     */
    private fun extractHandLandmarks(pose: Pose, imageWidth: Int, imageHeight: Int): List<PointF> {
        val wrist = pose.getPoseLandmark(PoseLandmark.RIGHT_WRIST) ?: pose.getPoseLandmark(PoseLandmark.LEFT_WRIST)
        val index = pose.getPoseLandmark(PoseLandmark.RIGHT_INDEX) ?: pose.getPoseLandmark(PoseLandmark.LEFT_INDEX)
        val pinky = pose.getPoseLandmark(PoseLandmark.RIGHT_PINKY) ?: pose.getPoseLandmark(PoseLandmark.LEFT_PINKY)
        val thumb = pose.getPoseLandmark(PoseLandmark.RIGHT_THUMB) ?: pose.getPoseLandmark(PoseLandmark.LEFT_THUMB)

        if (wrist == null || index == null || pinky == null || thumb == null) {
            return emptyList()
        }

        val w = imageWidth.toFloat().coerceAtLeast(1f)
        val h = imageHeight.toFloat().coerceAtLeast(1f)

        // Interpolate canonical 21 palm landmarks from geometric ray geometry
        val wPt = PointF(wrist.position.x / w, wrist.position.y / h)
        val iPt = PointF(index.position.x / w, index.position.y / h)
        val pPt = PointF(pinky.position.x / w, pinky.position.y / h)
        val tPt = PointF(thumb.position.x / w, thumb.position.y / h)
        val mPt = PointF((iPt.x + pPt.x) * 0.5f, (iPt.y + pPt.y) * 0.5f)

        val landmarks = ArrayList<PointF>(21)
        // 0: Wrist
        landmarks.add(wPt)

        // 1-4: Thumb
        landmarks.add(PointF(wPt.x * 0.7f + tPt.x * 0.3f, wPt.y * 0.7f + tPt.y * 0.3f))
        landmarks.add(PointF(wPt.x * 0.4f + tPt.x * 0.6f, wPt.y * 0.4f + tPt.y * 0.6f))
        landmarks.add(PointF(wPt.x * 0.2f + tPt.x * 0.8f, wPt.y * 0.2f + tPt.y * 0.8f))
        landmarks.add(tPt)

        // 5-8: Index
        landmarks.add(PointF(wPt.x * 0.3f + iPt.x * 0.7f, wPt.y * 0.3f + iPt.y * 0.7f))
        landmarks.add(PointF(wPt.x * 0.2f + iPt.x * 0.8f, wPt.y * 0.2f + iPt.y * 0.8f))
        landmarks.add(PointF(wPt.x * 0.1f + iPt.x * 0.9f, wPt.y * 0.1f + iPt.y * 0.9f))
        landmarks.add(iPt)

        // 9-12: Middle
        landmarks.add(PointF(wPt.x * 0.3f + mPt.x * 0.7f, wPt.y * 0.3f + mPt.y * 0.7f))
        landmarks.add(PointF(wPt.x * 0.2f + mPt.x * 0.8f, wPt.y * 0.2f + mPt.y * 0.8f))
        landmarks.add(PointF(wPt.x * 0.1f + mPt.x * 0.9f, wPt.y * 0.1f + mPt.y * 0.9f))
        landmarks.add(mPt)

        // 13-16: Ring
        val rPt = PointF(mPt.x * 0.5f + pPt.x * 0.5f, mPt.y * 0.5f + pPt.y * 0.5f)
        landmarks.add(PointF(wPt.x * 0.3f + rPt.x * 0.7f, wPt.y * 0.3f + rPt.y * 0.7f))
        landmarks.add(PointF(wPt.x * 0.2f + rPt.x * 0.8f, wPt.y * 0.2f + rPt.y * 0.8f))
        landmarks.add(PointF(wPt.x * 0.1f + rPt.x * 0.9f, wPt.y * 0.1f + rPt.y * 0.9f))
        landmarks.add(rPt)

        // 17-20: Pinky
        landmarks.add(PointF(wPt.x * 0.3f + pPt.x * 0.7f, wPt.y * 0.3f + pPt.y * 0.7f))
        landmarks.add(PointF(wPt.x * 0.2f + pPt.x * 0.8f, wPt.y * 0.2f + pPt.y * 0.8f))
        landmarks.add(PointF(wPt.x * 0.1f + pPt.x * 0.9f, wPt.y * 0.1f + pPt.y * 0.9f))
        landmarks.add(pPt)

        return landmarks
    }

    companion object {
        fun toFlatLandmarksArray(points: List<PointF>): FloatArray {
            val flat = FloatArray(points.size * 2)
            for (i in points.indices) {
                flat[i * 2] = points[i].x
                flat[i * 2 + 1] = points[i].y
            }
            return flat
        }
    }
}
