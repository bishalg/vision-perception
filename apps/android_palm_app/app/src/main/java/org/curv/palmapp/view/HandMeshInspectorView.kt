package org.curv.palmapp.view

import android.graphics.PointF
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.curv.overlay.palm.HandBiometricsCalculator

/**
 * 3D topography and surface mesh inspector — parity with iOS HandMeshInspectorView.
 */
@Composable
fun HandMeshInspectorView(
    landmarks: List<PointF>,
    padDepths: Map<String, Float> = emptyMap(),
    referencePalmWidthMm: Float = 80.0f,
    modifier: Modifier = Modifier
) {
    var showWireframe by remember { mutableStateOf(false) }
    var showCalipers by remember { mutableStateOf(true) }

    val biometrics = remember(landmarks, padDepths, referencePalmWidthMm) {
        if (landmarks.size >= 21) {
            val lms = landmarks.map { listOf(it.x, it.y) }
            HandBiometricsCalculator.compute(lms, referencePalmWidthMm = referencePalmWidthMm, padDepthMm = padDepths)
        } else null
    }

    Box(modifier = modifier.fillMaxSize().background(Color.Black)) {
        if (landmarks.size >= 21) {
            Canvas(modifier = Modifier.fillMaxSize()) {
                val w = size.width
                val h = size.height
                fun map(p: PointF) = Offset(p.x * w, p.y * h)

                // Triangulated hand mesh wireframe / surface
                val chains = listOf(
                    listOf(1, 2, 3, 4),
                    listOf(5, 6, 7, 8),
                    listOf(9, 10, 11, 12),
                    listOf(13, 14, 15, 16),
                    listOf(17, 18, 19, 20)
                )
                val palmCenter = Offset(w * 0.5f, h * 0.6f)

                // Palm triangulation
                val palmRing = listOf(0, 1, 2, 5, 9, 13, 17, 0)
                for (i in 0 until palmRing.size - 1) {
                    val p1 = map(landmarks[palmRing[i]])
                    val p2 = map(landmarks[palmRing[i + 1]])
                    val path = Path().apply {
                        moveTo(palmCenter.x, palmCenter.y)
                        lineTo(p1.x, p1.y)
                        lineTo(p2.x, p2.y)
                        close()
                    }
                    if (showWireframe) {
                        drawPath(path, Color(0xFF007AFF).copy(alpha = 0.4f), style = Stroke(width = 2f))
                    } else {
                        drawPath(path, Color(0xFF34C759).copy(alpha = 0.25f))
                    }
                }

                // Digits
                for (chain in chains) {
                    for (j in 0 until chain.size - 1) {
                        val p1 = map(landmarks[chain[j]])
                        val p2 = map(landmarks[chain[j + 1]])
                        val path = Path().apply {
                            moveTo(palmCenter.x, palmCenter.y)
                            lineTo(p1.x, p1.y)
                            lineTo(p2.x, p2.y)
                            close()
                        }
                        if (showWireframe) {
                            drawPath(path, Color(0xFF007AFF).copy(alpha = 0.35f), style = Stroke(width = 2f))
                        } else {
                            drawPath(path, Color(0xFF34C759).copy(alpha = 0.2f))
                        }
                    }
                }

                if (showCalipers) {
                    for ((idx, pt) in landmarks.withIndex()) {
                        val c = map(pt)
                        val color = if (idx == 8 || idx == 16) Color(0xFF34C759) else Color.White
                        drawCircle(color, radius = 6f, center = c)
                    }
                }
            }

            // Overlay Legend Card
            Column(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .padding(16.dp)
                    .background(Color(0xCC1A1A1A), RoundedCornerShape(16.dp))
                    .padding(16.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("3D Topography & Calipers", color = Color.White, fontSize = 14.sp)
                    biometrics?.ratios?.ratio2D4D?.let {
                        Text(String.format("2D:4D: %.3f", it), color = Color(0xFF34C759), fontSize = 14.sp)
                    }
                }
                Spacer(modifier = Modifier.height(8.dp))
                // Elevation heatmap gradient bar
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("-3mm", color = Color.Gray, fontSize = 10.sp)
                    Spacer(modifier = Modifier.width(4.dp))
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(6.dp)
                            .background(
                                Brush.horizontalGradient(
                                    listOf(Color(0xFF1E63D8), Color(0xFF34C759), Color(0xFFE5C033), Color(0xFFD63B3B))
                                ),
                                RoundedCornerShape(3.dp)
                            )
                    )
                    Spacer(modifier = Modifier.width(4.dp))
                    Text("+6mm", color = Color.Gray, fontSize = 10.sp)
                }
            }
        }
    }
}
