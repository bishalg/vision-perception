package org.curv.palmapp.view

import android.content.Intent
import android.graphics.PointF
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.curv.overlay.palm.HandBiometricsCalculator
import org.curv.overlay.palm.HandBiometricsReport
import org.curv.overlay.palm.HandSide

/**
 * Scientific anthropometric report view — parity with iOS AnthropometrySummaryView.
 */
@Composable
fun AnthropometrySummaryView(
    landmarks: List<PointF>,
    padDepths: Map<String, Float> = emptyMap(),
    referencePalmWidthMm: Float = 80.0f,
    handSide: HandSide = HandSide.RIGHT,
    modifier: Modifier = Modifier
) {
    val context = LocalContext.current
    val biometrics = remember(landmarks, padDepths, referencePalmWidthMm, handSide) {
        if (landmarks.size >= 21) {
            val lms = landmarks.map { listOf(it.x, it.y) }
            HandBiometricsCalculator.compute(
                landmarks = lms,
                referencePalmWidthMm = referencePalmWidthMm,
                padDepthMm = padDepths,
                handSide = handSide.name.lowercase()
            )
        } else null
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color.Black)
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        if (biometrics != null) {
            // 1. Digit Ratios Card
            Card(
                colors = CardDefaults.cardColors(containerColor = Color(0xFF1C1C1E)),
                shape = RoundedCornerShape(16.dp),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Digit Ratios & Biomarkers", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp)
                        Text("2D:4D Biomarker", color = Color(0xFF34C759), fontSize = 12.sp)
                    }

                    Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(
                            String.format("%.3f", biometrics.ratios.ratio2D4D),
                            color = Color.White,
                            fontSize = 36.sp,
                            fontWeight = FontWeight.Bold
                        )
                        Text("Index (2D) / Ring (4D)", color = Color.Gray, fontSize = 14.sp)
                    }

                    Divider(color = Color(0x33FFFFFF))

                    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Column {
                            Text("2D:3D (Index / Middle)", color = Color.Gray, fontSize = 11.sp)
                            Text(String.format("%.3f", biometrics.ratios.ratio2D3D), color = Color.White, fontWeight = FontWeight.SemiBold)
                        }
                        Column {
                            Text("3D:4D (Middle / Ring)", color = Color.Gray, fontSize = 11.sp)
                            Text(String.format("%.3f", biometrics.ratios.ratio3D4D), color = Color.White, fontWeight = FontWeight.SemiBold)
                        }
                    }
                }
            }

            // 2. Palmar Morphometry
            Card(
                colors = CardDefaults.cardColors(containerColor = Color(0xFF1C1C1E)),
                shape = RoundedCornerShape(16.dp),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text("Palmar Morphometry", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp)
                    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Column {
                            Text("Breadth", color = Color.Gray, fontSize = 11.sp)
                            Text(String.format("%.1f mm", biometrics.palm.metacarpalBreadthMm), color = Color.White)
                        }
                        Column {
                            Text("Palm Length", color = Color.Gray, fontSize = 11.sp)
                            Text(String.format("%.1f mm", biometrics.palm.palmLengthMm), color = Color.White)
                        }
                        Column {
                            Text("Hand Length", color = Color.Gray, fontSize = 11.sp)
                            Text(String.format("%.1f mm", biometrics.palm.totalHandLengthMm), color = Color.White)
                        }
                    }
                }
            }

            // 3. Export Button
            Button(
                onClick = {
                    val sendIntent = Intent().apply {
                        action = Intent.ACTION_SEND
                        putExtra(Intent.EXTRA_TEXT, "2D:4D=${biometrics.ratios.ratio2D4D}, palm=${biometrics.palm.metacarpalBreadthMm}mm")
                        type = "text/plain"
                    }
                    context.startActivity(Intent.createChooser(sendIntent, "Export Biometrics"))
                },
                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF007AFF)),
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Export Scientific Biometrics", color = Color.White)
            }
        }
    }
}
