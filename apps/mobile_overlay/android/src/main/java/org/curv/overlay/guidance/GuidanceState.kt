package org.curv.overlay.guidance

sealed class GuidanceState {
    data object Searching : GuidanceState()
    data object CenterSubject : GuidanceState()
    data object MoveCloser : GuidanceState()
    data object MoveBack : GuidanceState()
    data object HoldFlat : GuidanceState()
    data object TooDark : GuidanceState()
    data object GlareDetected : GuidanceState()
    data object HoldSteady : GuidanceState()
    data class Ready(val countdownProgress: Float) : GuidanceState()

    val isReady: Boolean get() = this is Ready
    val countdownProgress: Float get() = (this as? Ready)?.countdownProgress ?: 0f
}

data class GuidanceEvaluation(
    val state: GuidanceState,
    val metrics: QualityMetrics
)
