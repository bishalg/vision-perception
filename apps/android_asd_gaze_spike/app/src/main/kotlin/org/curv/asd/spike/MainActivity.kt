package org.curv.asd.spike

import android.app.AlertDialog
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.CheckBox
import android.widget.LinearLayout
import android.widget.NumberPicker
import android.widget.RadioButton
import android.widget.RadioGroup
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.TextView
import androidx.activity.ComponentActivity
import org.curv.asd.core.GazeBenchCondition
import org.curv.asd.core.coreVersion
import org.curv.asd.core.gazeBenchConditionName
import org.curv.asd.core.gazeBenchConditions

/**
 * Setup screen of the WP-A.14 spike: back-end, BEN-4 condition (closed list from Rust),
 * participant number (no names), measured distance; the saved logs and a delete button.
 */
class MainActivity : ComponentActivity() {

    private lateinit var logsText: TextView
    private val conditions: List<GazeBenchCondition> by lazy { gazeBenchConditions() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val pad = (16 * resources.displayMetrics.density).toInt()
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(pad, pad * 2, pad, pad)
        }
        fun label(text: String, size: Float = 15f, color: Int = Color.WHITE) = TextView(this).apply {
            this.text = text
            textSize = size
            setTextColor(color)
            setPadding(0, pad / 2, 0, pad / 4)
        }

        col.addView(label("ASD gaze spike (WP-A.14)", 22f))
        col.addView(label(
            "Throw-away feasibility app. Adult volunteers only, written consent, no child data. " +
                "Logs are biometric data: never commit them; delete them from the phone after copying.",
            13f, 0xFFFFCC80.toInt(),
        ))
        val core = runCatching { coreVersion() }.getOrElse { "unavailable: ${it.message}" }
        col.addView(label("asd-core $core", 12f, Color.GRAY))

        col.addView(label("Back-end"))
        val group = RadioGroup(this)
        val mp = RadioButton(this).apply { text = "MediaPipe Face Landmarker (iris)"; setTextColor(Color.WHITE); id = View.generateViewId() }
        val ar = RadioButton(this).apply { text = "ARCore Augmented Faces"; setTextColor(Color.WHITE); id = View.generateViewId() }
        group.addView(mp)
        group.addView(ar)
        group.check(mp.id)
        col.addView(group)
        val gpu = CheckBox(this).apply { text = "MediaPipe GPU delegate (run both CPU and GPU once)"; setTextColor(Color.WHITE) }
        col.addView(gpu)

        col.addView(label("Condition (BEN-4)"))
        val condition = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@MainActivity, android.R.layout.simple_spinner_dropdown_item,
                conditions.map { gazeBenchConditionName(it) },
            )
        }
        col.addView(condition)

        col.addView(label("Participant number (1-20, from the paper consent sheet; no names)"))
        val participant = NumberPicker(this).apply { minValue = 1; maxValue = 20; value = 1 }
        col.addView(participant)

        col.addView(label("Measured distance, glass to outer eye corner (cm)"))
        val distance = NumberPicker(this).apply {
            minValue = 30; maxValue = 70; value = 50
        }
        col.addView(distance)

        col.addView(Button(this).apply {
            text = "Start"
            setOnClickListener {
                val backend = if (group.checkedRadioButtonId == ar.id) "arcore" else "mediapipe"
                startActivity(RunActivity.intent(
                    this@MainActivity, backend, gpu.isChecked,
                    conditions[condition.selectedItemPosition], participant.value, distance.value,
                ))
            }
        })

        col.addView(label("Saved logs"))
        logsText = label("", 12f, Color.LTGRAY)
        col.addView(logsText)
        col.addView(Button(this).apply {
            text = "Delete all logs on this phone"
            setOnClickListener {
                AlertDialog.Builder(this@MainActivity)
                    .setMessage("Delete every spike log on this phone? Copy them first (adb pull).")
                    .setPositiveButton("Delete") { _, _ ->
                        DeviceFacts.logDir(this@MainActivity).listFiles()?.forEach { it.delete() }
                        refreshLogs()
                    }
                    .setNegativeButton("Cancel", null)
                    .show()
            }
        })

        setContentView(ScrollView(this).apply { addView(col) })
    }

    override fun onResume() {
        super.onResume()
        refreshLogs()
    }

    private fun refreshLogs() {
        val dir = DeviceFacts.logDir(this)
        val runs = dir.listFiles { f -> f.name.endsWith(".meta.json") }?.sortedBy { it.name }.orEmpty()
        logsText.text = if (runs.isEmpty()) "none" else
            runs.joinToString("\n") { it.name.removeSuffix(".meta.json") } +
                "\n\nadb pull /sdcard/Android/data/$packageName/files/spike_logs/"
    }
}
