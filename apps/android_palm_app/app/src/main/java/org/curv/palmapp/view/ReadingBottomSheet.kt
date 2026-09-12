package org.curv.palmapp.view

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Language
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.curv.overlay.palm.HandSide
import org.curv.palmapp.model.AppLanguage
import org.curv.palmapp.model.CreaseSpline
import org.curv.palmapp.model.LocalizedStrings
import org.curv.palmapp.model.PalmAnalysisResponse
import org.curv.palmapp.model.SimplePalmReading

/**
 * Liquid-glass bottom sheet presenting sacred Samudrika Shastra palm interpretations.
 * Phase 3 of Palm Scanner workflow — 1:1 port of iOS SamudrikaReadingSheet.swift:
 * - Pure interpretive reading grounded in user-verified crease geometry
 * - Zero raw CV telemetry on the consumer screen
 * - Localized and hand-side contextualized
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReadingBottomSheet(
    result: PalmAnalysisResponse?,
    splines: List<CreaseSpline>,
    selectedLanguage: AppLanguage,
    handSide: HandSide = HandSide.RIGHT,
    onLanguageChange: (AppLanguage) -> Unit,
    onRetake: () -> Unit,
    onDismiss: () -> Unit
) {
    // Keep result in signature for API compat; the consumer screen shows no raw telemetry.
    @Suppress("UNUSED_PARAMETER")
    val _keep = result

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = Color(0xFF121214).copy(alpha = 0.95f),
        scrimColor = Color.Black.copy(alpha = 0.5f),
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp)
    ) {
        var showLangMenu by remember { mutableStateOf(false) }

        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 36.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            // Drag handle indicator.
            Box(
                Modifier.size(width = 44.dp, height = 5.dp)
                    .clip(CircleShape)
                    .background(Color.White.copy(alpha = 0.3f))
                    .align(Alignment.CenterHorizontally)
            )

            // Header & language picker.
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = LocalizedStrings.tr("sheet.title", selectedLanguage),
                        color = Color.White,
                        fontSize = 20.sp,
                        fontWeight = FontWeight.Bold
                    )
                    Text(
                        text = if (handSide == HandSide.LEFT) "Left Palm • Inherited Potential"
                        else "Right Palm • Active Manifestation",
                        color = Color.White.copy(alpha = 0.60f),
                        fontSize = 12.sp,
                        fontWeight = FontWeight.Medium
                    )
                }
                Box {
                    Row(
                        modifier = Modifier.clip(CircleShape)
                            .background(Color.White.copy(alpha = 0.1f))
                            .border(1.dp, Color.White.copy(alpha = 0.2f), CircleShape)
                            .clickable { showLangMenu = true }
                            .padding(horizontal = 10.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        Icon(Icons.Default.Language, null, tint = Color.White, modifier = Modifier.size(16.dp))
                        Text(
                            text = selectedLanguage.displayName,
                            color = Color.White,
                            fontSize = 13.sp,
                            fontWeight = FontWeight.Medium
                        )
                    }
                    DropdownMenu(expanded = showLangMenu, onDismissRequest = { showLangMenu = false }) {
                        AppLanguage.values().forEach { lang ->
                            DropdownMenuItem(
                                text = { Text(lang.displayName) },
                                onClick = {
                                    onLanguageChange(lang)
                                    showLangMenu = false
                                }
                            )
                        }
                    }
                }
            }

            // Crease line interpretations (liquid-glass cards).
            LazyColumn(
                verticalArrangement = Arrangement.spacedBy(14.dp),
                modifier = Modifier.heightIn(max = 380.dp)
            ) {
                items(splines, key = { it.id }) { spline ->
                    val reading = SimplePalmReading.readingText(spline, handSide)
                    GlassCard {
                        Column(
                            modifier = Modifier.fillMaxWidth().padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(10.dp)
                            ) {
                                val dot = Color(android.graphics.Color.parseColor(spline.lineType.hexColor))
                                Box(
                                    modifier = Modifier.size(12.dp).clip(CircleShape).background(dot)
                                )
                                Text(
                                    text = LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage),
                                    color = Color.White,
                                    fontSize = 16.sp,
                                    fontWeight = FontWeight.Bold
                                )
                                if (spline.isUserAdjusted) {
                                    Text(
                                        text = "verified",
                                        color = Color(0xFF34C759),
                                        fontSize = 10.sp,
                                        fontWeight = FontWeight.Bold,
                                        modifier = Modifier.clip(CircleShape)
                                            .background(Color(0xFF34C759).copy(alpha = 0.18f))
                                            .padding(horizontal = 6.dp, vertical = 2.dp)
                                    )
                                }
                                Spacer(Modifier.weight(1f))
                                Text(
                                    text = "${(spline.confidence * 100).toInt()}% clear",
                                    color = Color(0xFF34C759),
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.SemiBold
                                )
                            }
                            Text(
                                text = reading,
                                color = Color.White.copy(alpha = 0.92f),
                                fontSize = 14.sp,
                                lineHeight = 20.sp
                            )
                        }
                    }
                }
            }

            // Actions: Done + Retake.
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Button(
                    onClick = onDismiss,
                    modifier = Modifier.fillMaxWidth().padding(top = 10.dp),
                    shape = RoundedCornerShape(26.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF34C759))
                ) {
                    Text(
                        text = LocalizedStrings.tr("result.doneEdit", selectedLanguage),
                        color = Color.Black,
                        fontSize = 16.sp,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(vertical = 6.dp)
                    )
                }
                Row(
                    modifier = Modifier.fillMaxWidth().clickable {
                        onDismiss()
                        onRetake()
                    }.padding(vertical = 8.dp),
                    horizontalArrangement = Arrangement.Center,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(Icons.Default.Refresh, null, tint = Color.White.copy(alpha = 0.8f), modifier = Modifier.size(16.dp))
                    Text(
                        text = LocalizedStrings.tr("button.retake", selectedLanguage),
                        color = Color.White.copy(alpha = 0.8f),
                        fontSize = 14.sp,
                        fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.padding(start = 6.dp)
                    )
                }
            }
        }
    }
}

@Composable
fun GlassCard(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(20.dp))
            .background(Color.White.copy(alpha = 0.06f))
            .border(1.dp, Color.White.copy(alpha = 0.12f), RoundedCornerShape(20.dp))
    ) {
        content()
    }
}
