package org.curv.asd.spike

import android.os.SystemClock

/**
 * Puts the target schedule on the camera frame clock, so the log's `t_sec` and the header's
 * `startSec`/`endSec` share one time base (REP-1), as the iOS bench does with the ARKit clock.
 *
 * Camera timestamps on Android are either `elapsedRealtimeNanos` (REALTIME source) or
 * `System.nanoTime` (CLOCK_MONOTONIC, the usual UNKNOWN source); ARCore leaves its time base
 * undefined. On the first frame the host clock that the frame time is just behind (captured
 * 0..2 s ago) is chosen. If neither fits, the offset at the first frame is used and the run's
 * meta says `offsetEstimated` (target times then carry the first frame's delivery latency).
 * This is clock bookkeeping, not gaze maths.
 */
class FrameClock {
    @Volatile private var host: (() -> Long)? = null
    @Volatile private var offsetNs: Long = 0

    @Volatile var mode: String = "unset"
        private set

    val ready: Boolean get() = host != null

    @Synchronized
    fun onFrame(frameNs: Long) {
        if (host != null) return
        val candidates = listOf(
            "elapsedRealtime" to { SystemClock.elapsedRealtimeNanos() },
            "monotonic" to { System.nanoTime() },
        )
        val fit = candidates
            .map { (name, clock) -> Triple(name, clock, clock() - frameNs) }
            .filter { (_, _, lag) -> lag in -100_000_000L..2_000_000_000L }
            .minByOrNull { (_, _, lag) -> kotlin.math.abs(lag) }
        if (fit != null) {
            mode = fit.first
            offsetNs = 0
            host = fit.second
        } else {
            val clock = { SystemClock.elapsedRealtimeNanos() }
            mode = "offsetEstimated"
            offsetNs = frameNs - clock()
            host = clock
        }
    }

    /** Now, in seconds on the frame clock. */
    fun nowSec(): Double {
        val h = host ?: return Double.NaN
        return (h() + offsetNs) / 1e9
    }
}
