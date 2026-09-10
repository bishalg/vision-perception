package org.curv.overlay.guidance

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlin.math.atan2

/**
 * Device roll vs gravity (camera axis tilt).
 * rollDeg = atan2(gravity.x, -gravity.y) in degrees, 0 = device vertical.
 * Mirrors iOS DeviceTiltProvider so both platforms share sign + 0 meaning.
 */
class DeviceTiltMonitor(context: Context) : SensorEventListener {

    private val sensorManager = context.getSystemService(Context.SENSOR_SERVICE) as SensorManager
    private val gravitySensor: Sensor? =
        sensorManager.getDefaultSensor(Sensor.TYPE_GRAVITY)
            ?: sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER)

    private val _deviceRollDeg = MutableStateFlow(0f)
    val deviceRollDeg: StateFlow<Float> = _deviceRollDeg.asStateFlow()

    private var started = false
    private var smoothed = 0f

    fun start() {
        if (started) return
        gravitySensor?.let {
            sensorManager.registerListener(this, it, SensorManager.SENSOR_DELAY_UI)
            started = true
        }
    }

    fun stop() {
        if (!started) return
        sensorManager.unregisterListener(this)
        started = false
    }

    override fun onSensorChanged(event: SensorEvent) {
        val gx = event.values[0]
        val gy = event.values[1]
        // same math as iOS
        val roll = Math.toDegrees(atan2(gx.toDouble(), -gy.toDouble())).toFloat()
        smoothed = roll * 0.35f + smoothed * 0.65f
        _deviceRollDeg.value = smoothed
    }

    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit
}
