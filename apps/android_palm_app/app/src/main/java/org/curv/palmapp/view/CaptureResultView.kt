package org.curv.palmapp.view

import android.graphics.Bitmap
import android.graphics.PointF
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.curv.overlay.palm.HandSide
import org.curv.palmapp.model.AppLanguage
import org.curv.palmapp.model.CreaseOverlaySource
import org.curv.palmapp.model.CreaseSpline
import org.curv.palmapp.model.LocalizedStrings

/**
 * Post-capture review: photo + verified crease overlays.
 * Phase 2 of Palm Scanner workflow — 1:1 port of iOS CaptureResultView.swift:
 * - Clean display of detected crease polylines
 * - Top-right Edit capsule opens the dedicated [CreaseLineEditorScreen]
 * - Reachable close button, Retake / Analyze Reading bottom bar
 */
@Composable
fun CaptureResultView(
    bitmap: Bitmap?,
    landmarks: List<PointF>,
    splines: List<CreaseSpline>,
    selectedLanguage: AppLanguage,
    overlaySource: CreaseOverlaySource,
    handSide: HandSide = HandSide.RIGHT,
    onSplinesChange: (List<CreaseSpline>) -> Unit,
    onLanguageChange: (AppLanguage) -> Unit = {},
    onRetake: () -> Unit,
    onConfirmAdjustedLines: () -> Unit,
    onDismiss: () -> Unit,
    onProceedToAnalysis: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    var showEditor by remember { mutableStateOf(false) }
    var selectedSplineId by remember(splines) { mutableStateOf<String?>(null) }

    val needsManual = overlaySource == CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT
    val sourceSubtitle = if (needsManual) LocalizedStrings.tr("result.landmark", selectedLanguage)
    else LocalizedStrings.tr("result.native", selectedLanguage)

    fun proceed() {
        onConfirmAdjustedLines()
        onProceedToAnalysis?.invoke()
    }

    Box(modifier = modifier.fillMaxSize().background(Color.Black)) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Top safe-area navigation header.
            Row(
                modifier = Modifier.fillMaxWidth()
                    .background(Color.Black.copy(alpha = 0.8f))
                    .padding(top = 50.dp, start = 16.dp, end = 16.dp, bottom = 10.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Reachable close button.
                Box(
                    modifier = Modifier.size(44.dp).clip(CircleShape)
                        .clickable { onDismiss() },
                    contentAlignment = Alignment.Center
                ) {
                    Text("✕", color = Color.White.copy(alpha = 0.85f), fontSize = 22.sp)
                }
                Spacer(Modifier.weight(1f))
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        text = LocalizedStrings.tr("result.title", selectedLanguage),
                        color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold
                    )
                    Text(
                        text = sourceSubtitle,
                        color = Color.White.copy(alpha = 0.55f), fontSize = 11.sp
                    )
                }
                Spacer(Modifier.weight(1f))
                // Edit capsule (top-right).
                Row(
                    modifier = Modifier.clip(CircleShape)
                        .background(Color.White.copy(alpha = 0.14f))
                        .border(1.dp, Color.White.copy(alpha = 0.22f), CircleShape)
                        .clickable { showEditor = true }
                        .padding(horizontal = 12.dp, vertical = 7.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(5.dp)
                ) {
                    Icon(Icons.Default.Tune, null, tint = Color.White, modifier = Modifier.size(14.dp))
                    Text(
                        text = LocalizedStrings.tr("result.edit", selectedLanguage),
                        color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.SemiBold
                    )
                }
            }

            // Captured image + detected creases canvas.
            BoxWithConstraints(
                modifier = Modifier.weight(1f).fillMaxWidth().background(Color.Black)
            ) {
                val density = androidx.compose.ui.platform.LocalDensity.current
                val viewSize = Size(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat())
                val imgSize = if (bitmap != null) Size(bitmap.width.toFloat(), bitmap.height.toFloat()) else viewSize
                val drawRect = ImageCoordinateMapper.imageRect(imgSize, viewSize, ImageCoordinateMapper.ContentMode.FIT)
                val rectWdp = with(density) { drawRect.width.toDp() }
                val rectHdp = with(density) { drawRect.height.toDp() }
                Box(modifier = Modifier.fillMaxSize()) {
                    if (bitmap != null) {
                        Image(
                            bitmap = bitmap.asImageBitmap(),
                            contentDescription = null,
                            modifier = Modifier.width(rectWdp).height(rectHdp).align(Alignment.Center),
                            contentScale = ContentScale.FillBounds
                        )
                    } else {
                        Text(
                            "No photo",
                            color = Color.White.copy(alpha = 0.5f),
                            modifier = Modifier.align(Alignment.Center)
                        )
                    }
                    // Soft top/bottom scrim for legibility.
                    Box(
                        modifier = Modifier.fillMaxSize().background(
                            Brush.verticalGradient(
                                listOf(
                                    Color.Black.copy(alpha = 0.2f),
                                    Color.Transparent,
                                    Color.Black.copy(alpha = 0.3f)
                                )
                            )
                        )
                    )
                    HandOutlineOverlay(
                        landmarks = landmarks,
                        imageRect = drawRect,
                        modifier = Modifier.fillMaxSize()
                    )
                    CreaseSplineOverlayView(
                        splines = splines,
                        imageRect = drawRect,
                        selectedSplineId = selectedSplineId,
                        isEditing = false,
                        useScaffoldStyle = needsManual,
                        modifier = Modifier.fillMaxSize()
                    )
                }
            }

            // Bottom verification summary card.
            Column(
                modifier = Modifier.fillMaxWidth()
                    .clip(RoundedCornerShape(topStart = 24.dp, topEnd = 24.dp))
                    .background(Color(0xFF1F1F22))
                    .padding(horizontal = 20.dp)
                    .padding(top = 16.dp, bottom = 28.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(14.dp)
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = "Detected Creases (${splines.size})",
                        color = Color.White.copy(alpha = 0.9f),
                        fontSize = 14.sp, fontWeight = FontWeight.Bold
                    )
                    Spacer(Modifier.weight(1f))
                    Text(
                        text = "Edit Points",
                        color = Color(0xFF34C759),
                        fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clickable { showEditor = true }
                    )
                }

                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    splines.forEach { spline ->
                        CreaseLineRow(
                            spline = spline,
                            selectedLanguage = selectedLanguage,
                            onClick = {
                                selectedSplineId = spline.id
                                showEditor = true
                            }
                        )
                    }
                }

                // Bottom bar: small Retake left + large Analyze CTA right.
                Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Button(
                        onClick = onRetake,
                        modifier = Modifier.width(110.dp).height(52.dp),
                        shape = RoundedCornerShape(26.dp),
                        colors = ButtonDefaults.buttonColors(containerColor = Color.White.copy(alpha = 0.10f)),
                        border = androidx.compose.foundation.BorderStroke(1.dp, Color.White.copy(alpha = 0.18f))
                    ) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Icon(Icons.Default.Refresh, null, tint = Color.White, modifier = Modifier.size(16.dp))
                            Text(
                                text = LocalizedStrings.tr("result.retake", selectedLanguage),
                                color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.SemiBold
                            )
                        }
                    }
                    Button(
                        onClick = { proceed() },
                        enabled = splines.isNotEmpty(),
                        modifier = Modifier.weight(1f).height(52.dp),
                        shape = RoundedCornerShape(26.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = Color(0xFF34C759),
                            disabledContainerColor = Color(0xFF34C759).copy(alpha = 0.4f)
                        )
                    ) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Icon(Icons.Default.AutoAwesome, null, tint = Color.Black, modifier = Modifier.size(16.dp))
                            Text(
                                text = LocalizedStrings.tr("result.continueReading", selectedLanguage),
                                color = Color.Black, fontSize = 15.sp, fontWeight = FontWeight.Bold
                            )
                        }
                    }
                }
            }
        }

        // Dedicated precision editor (full-screen).
        if (showEditor) {
            CreaseLineEditorScreen(
                bitmap = bitmap,
                splines = splines,
                selectedLanguage = selectedLanguage,
                onDismiss = { showEditor = false },
                onDone = { updated ->
                    onSplinesChange(updated)
                    showEditor = false
                },
                modifier = Modifier.fillMaxSize()
            )
        }
    }
}

@Composable
private fun CreaseLineRow(
    spline: CreaseSpline,
    selectedLanguage: AppLanguage,
    onClick: () -> Unit
) {
    val color = parseResultHex(spline.lineType.hexColor)
    Row(
        modifier = Modifier.fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(Color.White.copy(alpha = 0.04f))
            .clickable { onClick() }
            .padding(vertical = 8.dp, horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Box(
            Modifier.size(width = 18.dp, height = 5.dp).clip(CircleShape).background(color)
        )
        Text(
            text = LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage),
            color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold
        )
        if (spline.isUserAdjusted) {
            Text("•", color = Color(0xFF34C759), fontSize = 12.sp)
            Text(
                text = LocalizedStrings.tr("result.adjusted", selectedLanguage),
                color = Color(0xFF34C759), fontSize = 11.sp, fontWeight = FontWeight.Medium
            )
        }
        Spacer(Modifier.weight(1f))
        Text(
            text = "${spline.points.size} pts",
            color = Color.White.copy(alpha = 0.55f), fontSize = 11.sp
        )
        Text(
            text = "${(spline.confidence * 100).toInt()}%",
            color = Color(0xFF34C759), fontSize = 12.sp, fontWeight = FontWeight.Bold,
            modifier = Modifier.clip(CircleShape)
                .background(Color(0xFF34C759).copy(alpha = 0.14f))
                .padding(horizontal = 7.dp, vertical = 3.dp)
        )
        Icon(Icons.Default.ChevronRight, null, tint = Color.White.copy(alpha = 0.25f), modifier = Modifier.size(14.dp))
    }
}

private fun parseResultHex(hex: String): Color {
    val c = hex.removePrefix("#").toLongOrNull(16) ?: 0xFFFFFF
    return Color(0xFF000000 or c)
}
