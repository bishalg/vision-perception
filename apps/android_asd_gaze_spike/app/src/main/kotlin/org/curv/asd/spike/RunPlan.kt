package org.curv.asd.spike

import org.curv.asd.core.GazeOrientation
import org.curv.asd.core.GazeTargetKind
import org.curv.asd.core.gazeDefaultConfigJson
import org.json.JSONArray
import org.json.JSONObject

/** One target of the fixed spike sequence (no gaze feedback, fixed timing, as BEN-2). */
data class PlannedTarget(
    val kind: GazeTargetKind,
    val xNorm: Double,
    val yNorm: Double,
    val dwellSec: Double,
    val label: String,
)

/**
 * The run plan, read from the Rust gaze config (`gazeDefaultConfigJson`), never hard-coded:
 * the 9-point calibration layout (CAL-7 `layout9`), then the 13-point bench grid (VAL-5
 * `benchLayout13`) as validation targets, then a free-viewing clip. Target windows are the
 * config's `settleMs + collectMs` for each kind.
 */
class RunPlan private constructor(
    val targets: List<PlannedTarget>,
    /** The config root as Rust resolved it for this model (geometry source, geometry). */
    val config: JSONObject,
) {
    companion object {
        const val LEAD_IN_SEC = 2.0
        const val GAP_SEC = 0.25
        const val FREE_VIEW_SEC = 60.0

        fun load(modelIdentifier: String): RunPlan {
            val root = JSONObject(gazeDefaultConfigJson(modelIdentifier, GazeOrientation.PORTRAIT))
            val params = root.getJSONObject("params")
            val cal = params.getJSONObject("calibration")
            val val_ = params.getJSONObject("validation")
            val calDwell = (cal.getDouble("settleMs") + cal.getDouble("collectMs")) / 1000.0
            val valDwell = (val_.getDouble("settleMs") + val_.getDouble("collectMs")) / 1000.0
            val out = ArrayList<PlannedTarget>()
            points(cal.getJSONArray("layout9")).forEachIndexed { i, p ->
                out += PlannedTarget(GazeTargetKind.CALIBRATION, p.first, p.second, calDwell, "cal9-${i + 1}")
            }
            points(val_.getJSONArray("benchLayout13")).forEachIndexed { i, p ->
                out += PlannedTarget(GazeTargetKind.VALIDATION, p.first, p.second, valDwell, "grid13-${i + 1}")
            }
            return RunPlan(out, root)
        }

        private fun points(a: JSONArray): List<Pair<Double, Double>> =
            (0 until a.length()).map { a.getJSONArray(it).let { p -> p.getDouble(0) to p.getDouble(1) } }
    }
}
