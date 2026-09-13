package org.curv.overlay.palm

import org.curv.overlay.camera.PhotoDepthMap
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * Samples per-mount mean depth from a hardware depth map.
 *
 * Pure function over plain arrays (no Android framework UI or Camera APIs),
 * so it is fully unit-testable on JVM.
 *
 * Coordinate contract: `landmarks` are normalized points in the upright
 * still-image space, and the depth map is registered to that same still.
 *
 * Robustness rules: samples outside 0.05...3.0 m are background (rejected);
 * a mount needs >= 3 valid samples; the frame needs >= 4 valid mounts.
 * Anything less returns null and the estimator falls back automatically.
 */
object MountDepthSampler {
    /**
     * Single Kotlin source of truth for mount probe indices. Must match
     * `mountAnchors()` in MountReliefAnalyzer.cpp and `MountDepthSampler.swift`.
     */
    val anchors: Map<String, List<Int>> = mapOf(
        "venus" to listOf(1, 2, 5),
        "jupiter" to listOf(5, 6, 7),
        "saturn" to listOf(9, 10, 11),
        "apollo" to listOf(13, 14, 15),
        "mercury" to listOf(17, 18, 19),
        "luna" to listOf(0, 17, 18),
        "mars" to listOf(5, 9, 13, 17)
    )

    const val MIN_VALID_SAMPLES_PER_MOUNT = 3
    const val MIN_VALID_MOUNTS = 4
    private const val MIN_DEPTH_M = 0.05f
    private const val MAX_DEPTH_M = 3.0f

    fun sampleMountMeansMm(
        depthMap: PhotoDepthMap,
        landmarks: List<List<Float>>,
        minValidMounts: Int = MIN_VALID_MOUNTS
    ): Map<String, Float>? {
        if (!depthMap.isValid || landmarks.size < 21) return null
        val radius = max(1, min(depthMap.width, depthMap.height) / 48)
        val means = mutableMapOf<String, Float>()

        for ((mount, idxs) in anchors) {
            val samples = mutableListOf<Float>()
            for (idx in idxs) {
                if (idx in landmarks.indices && landmarks[idx].size >= 2) {
                    val cx = (landmarks[idx][0] * depthMap.width).roundToInt()
                    val cy = (landmarks[idx][1] * depthMap.height).roundToInt()
                    for (dy in -radius..radius) {
                        for (dx in -radius..radius) {
                            val x = cx + dx
                            val y = cy + dy
                            if (x in 0 until depthMap.width && y in 0 until depthMap.height) {
                                val v = depthMap.meters[y * depthMap.width + x]
                                if (v.isFinite() && v in MIN_DEPTH_M..MAX_DEPTH_M) {
                                    samples.add(v)
                                }
                            }
                        }
                    }
                }
            }
            if (samples.size >= MIN_VALID_SAMPLES_PER_MOUNT) {
                samples.sort()
                val medianM = samples[samples.size / 2]
                means[mount] = medianM * 1000f
            }
        }
        return if (means.size >= minValidMounts) means else null
    }
}
