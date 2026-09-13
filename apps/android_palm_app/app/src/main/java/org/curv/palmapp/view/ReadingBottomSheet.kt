package org.curv.palmapp.view

import androidx.compose.foundation.background
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Language
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import org.curv.overlay.palm.HandSide
import org.curv.palmapp.model.AppLanguage
import org.curv.palmapp.model.CreaseSpline
import org.curv.palmapp.model.LocalizedStrings
import org.curv.palmapp.model.PalmAnalysisResponse
import org.curv.palmapp.model.SimplePalmReading

/**
 * Samudrika Shastra palm interpretations.
 * Phase 3 of Palm Scanner workflow — native Material3 bottom sheet:
 * - System sheet chrome (grab handle, scrim, shape all come from M3).
 * - One [ElevatedCard] per crease line with headline + reading body.
 * - Theme colors + typography: follows light/dark mode like any native sheet.
 * - Pure interpretive reading grounded in user-verified crease geometry.
 * - Zero raw CV telemetry on the consumer screen.
 * - Localized and hand-side contextualized.
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

    // Native sheet chrome: default drag handle, scrim, shape, and surface color.
    ModalBottomSheet(onDismissRequest = onDismiss) {
        var showLangMenu by remember { mutableStateOf(false) }

        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 36.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            // Header & language picker.
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = LocalizedStrings.tr("sheet.title", selectedLanguage),
                        style = MaterialTheme.typography.headlineSmall
                    )
                    Text(
                        text = if (handSide == HandSide.LEFT) "Left Palm • Inherited Potential"
                        else "Right Palm • Active Manifestation",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                IconButton(onClick = { showLangMenu = true }) {
                    Icon(Icons.Filled.Language, contentDescription = selectedLanguage.displayName)
                }
                DropdownMenu(expanded = showLangMenu, onDismissRequest = { showLangMenu = false }) {
                    AppLanguage.values().forEach { lang ->
                        DropdownMenuItem(
                            text = { Text(lang.displayName) },
                            trailingIcon = if (lang == selectedLanguage) {
                                { Text("✓", color = MaterialTheme.colorScheme.primary) }
                            } else null,
                            onClick = {
                                onLanguageChange(lang)
                                showLangMenu = false
                            }
                        )
                    }
                }
            }

            // Crease line interpretations, one native card per line.
            LazyColumn(
                verticalArrangement = Arrangement.spacedBy(14.dp),
                modifier = Modifier.heightIn(max = 380.dp)
            ) {
                items(splines, key = { it.id }) { spline ->
                    val reading = SimplePalmReading.readingText(
                        spline, handSide,
                        SimplePalmReading.LineReports.from(result?.domainPayload)
                    )
                    ElevatedCard(modifier = Modifier.fillMaxWidth()) {
                        Column(
                            modifier = Modifier.fillMaxWidth().padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(10.dp)
                            ) {
                                LineDot(hexColor = spline.lineType.hexColor)
                                Text(
                                    text = LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage),
                                    style = MaterialTheme.typography.titleMedium
                                )
                                if (spline.isUserAdjusted) {
                                    Text(
                                        text = LocalizedStrings.tr("result.verified", selectedLanguage),
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.primary
                                    )
                                }
                                Spacer(Modifier.weight(1f))
                                Text(
                                    text = "${(spline.confidence * 100).toInt()}% clear",
                                    style = MaterialTheme.typography.labelLarge,
                                    color = MaterialTheme.colorScheme.primary
                                )
                            }
                            Text(
                                text = reading,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                        }
                    }
                }
            }

            // Actions: Done + Retake with native button styles.
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Button(
                    onClick = onDismiss,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = LocalizedStrings.tr("result.doneEdit", selectedLanguage),
                        modifier = Modifier.padding(vertical = 6.dp)
                    )
                }
                TextButton(
                    onClick = {
                        onDismiss()
                        onRetake()
                    },
                    modifier = Modifier.align(Alignment.CenterHorizontally)
                ) {
                    Icon(Icons.Filled.Refresh, null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.size(6.dp))
                    Text(LocalizedStrings.tr("button.retake", selectedLanguage))
                }
            }
        }
    }
}

/** Brand-color swatch dot identifying a crease line. */
@Composable
private fun LineDot(hexColor: String, modifier: Modifier = Modifier) {
    val c = hexColor.removePrefix("#").toLongOrNull(16) ?: 0xFFFFFF
    Box(
        modifier = modifier.size(12.dp)
            .clip(CircleShape)
            .background(Color(0xFF000000 or c))
    )
}
