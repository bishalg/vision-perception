package org.curv.asd.spike

import android.app.Activity
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Build
import android.os.PowerManager
import android.util.DisplayMetrics
import org.json.JSONObject
import java.io.File

/** Device, display and thermal facts for `.meta.json` (no personal data). */
object DeviceFacts {
    /** The header's `deviceModel` (Android has no `utsname.machine`; `Build.MODEL` is closest). */
    val modelIdentifier: String get() = Build.MODEL ?: "unknown"

    fun device(): JSONObject = JSONObject()
        .put("manufacturer", Build.MANUFACTURER)
        .put("model", Build.MODEL)
        .put("device", Build.DEVICE)
        .put("hardware", Build.HARDWARE)
        .put("socModel", if (Build.VERSION.SDK_INT >= 31) Build.SOC_MODEL else JSONObject.NULL)
        .put("sdkInt", Build.VERSION.SDK_INT)
        .put("release", Build.VERSION.RELEASE)

    /** Physical display size from the reported dpi (a fact for the future Android geometry table). */
    @Suppress("DEPRECATION")
    fun display(activity: Activity, viewW: Int, viewH: Int): JSONObject {
        val m = DisplayMetrics()
        activity.windowManager.defaultDisplay.getRealMetrics(m)
        return JSONObject()
            .put("realPixels", org.json.JSONArray(listOf(m.widthPixels, m.heightPixels)))
            .put("xdpi", m.xdpi.toDouble())
            .put("ydpi", m.ydpi.toDouble())
            .put("widthMmFromDpi", m.widthPixels / m.xdpi * 25.4)
            .put("heightMmFromDpi", m.heightPixels / m.ydpi * 25.4)
            .put("stimulusViewPixels", org.json.JSONArray(listOf(viewW, viewH)))
            .put("note", "dpi values are what the OEM reports; measure the active area and camera offset with calipers for the sheet")
    }

    fun thermal(activity: Activity): JSONObject {
        val o = JSONObject()
        val battery = activity.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        battery?.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, Int.MIN_VALUE)
            ?.takeIf { it != Int.MIN_VALUE }
            ?.let { o.put("batteryTempC", it / 10.0) }
        battery?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)?.takeIf { it >= 0 }?.let { o.put("batteryLevel", it) }
        if (Build.VERSION.SDK_INT >= 29) {
            val pm = activity.getSystemService(PowerManager::class.java)
            o.put("thermalStatus", pm.currentThermalStatus)
        }
        return o
    }

    fun logDir(activity: Activity): File =
        File(activity.getExternalFilesDir(null) ?: activity.filesDir, "spike_logs").apply { mkdirs() }
}
