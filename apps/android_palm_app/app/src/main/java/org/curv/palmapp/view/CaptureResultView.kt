package org.curv.palmapp.view

import android.graphics.Bitmap
import android.graphics.PointF
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.curv.overlay.palm.HandSide
import org.curv.palmapp.model.AppLanguage
import org.curv.palmapp.model.CreaseOverlaySource
import org.curv.palmapp.model.CreaseSpline
import org.curv.palmapp.model.LocalizedStrings
import org.curv.palmapp.ui.theme.PalmAppTheme

enum class ResultInspectorTab(val label: String) {
    CREASES("2D Creases"),
    TOPOGRAPHY("3D Topography"),
    ANTHROPOMETRY("Anthropometry")
}

/**
 * Post-capture review: photo + verified crease overlays.
 * Phase 2 of Palm Scanner workflow — native Material3 structure:
 * - [TopAppBar] with close navigation icon, title/subtitle, and Edit action.
 * - Native [ListItem] rows for detected crease lines; tap to open the editor.
 * - [FilledTonalButton] Retake + [Button] Analyze in the bottom bar.
 * - The photo canvas itself stays custom (no native equivalent for overlays).
 * - Wrapped in a dark theme: review is a media surface, dark like the gallery.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CaptureResultView(
    bitmap: Bitmap?,
    landmarks: List<PointF>,
    splines: List<CreaseSpline>,
    selectedLanguage: AppLanguage,
    overlaySource: CreaseOverlaySource,
    handSide: HandSide = HandSide.RIGHT,
    padDepths: Map<String, Float> = emptyMap(),
    onSplinesChange: (List<CreaseSpline>) -> Unit,
    onLanguageChange: (AppLanguage) -> Unit = {},
    onRetake: () -> Unit,
    onConfirmAdjustedLines: () -> Unit,
    onDismiss: () -> Unit,
    onProceedToAnalysis: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    var selectedTab by remember { mutableStateOf(ResultInspectorTab.CREASES) }
    var showEditor by remember { mutableStateOf(false) }
    var selectedSplineId by remember(splines) { mutableStateOf<String?>(null) }

    val needsManual = overlaySource == CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT
    val sourceSubtitle = if (needsManual) LocalizedStrings.tr("result.landmark", selectedLanguage)
    else LocalizedStrings.tr("result.native", selectedLanguage)

    fun proceed() {
        onConfirmAdjustedLines()
        onProceedToAnalysis?.invoke()
    }

    PalmAppTheme() {
        Scaffold(
            modifier = modifier.fillMaxSize(),
            containerColor = Color.Black,
            topBar = {
                TopAppBar(
                    title = {
                        Column {
                            Text(
                                text = LocalizedStrings.tr("result.title", selectedLanguage),
                                style = MaterialTheme.typography.titleLarge
                            )
                            Text(
                                text = sourceSubtitle,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    },
                    navigationIcon = {
                        IconButton(onClick = onDismiss) {
                            Icon(Icons.Filled.Close, contentDescription = null)
                        }
                    },
                    actions = {
                        TextButton(onClick = { showEditor = true }) {
                            Icon(Icons.Filled.Tune, null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.size(4.dp))
                            Text(LocalizedStrings.tr("result.edit", selectedLanguage))
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = Color.Black.copy(alpha = 0.8f),
                        titleContentColor = Color.White,
                        navigationIconContentColor = Color.White,
                        actionIconContentColor = Color.White
                    )
                )
            },
            bottomBar = {
                // Retake / Analyze Reading action bar with native button styles.
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    FilledTonalButton(
                        onClick = onRetake,
                        modifier = Modifier.width(110.dp).height(52.dp)
                    ) {
                        Icon(Icons.Filled.Refresh, null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.size(6.dp))
                        Text(LocalizedStrings.tr("result.retake", selectedLanguage))
                    }
                    Button(
                        onClick = { proceed() },
                        enabled = splines.isNotEmpty(),
                        modifier = Modifier.weight(1f).height(52.dp)
                    ) {
                        Icon(Icons.Filled.AutoAwesome, null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.size(8.dp))
                        Text(
                            text = LocalizedStrings.tr("result.continueReading", selectedLanguage),
                            fontWeight = FontWeight.Bold
                        )
                    }
                }
            }
        ) { innerPadding ->
            Column(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
                // Segmented tab row
                TabRow(
                    selectedTabIndex = selectedTab.ordinal,
                    containerColor = Color.Black.copy(alpha = 0.85f),
                    contentColor = Color.White
                ) {
                    ResultInspectorTab.values().forEach { tab ->
                        Tab(
                            selected = selectedTab == tab,
                            onClick = { selectedTab = tab },
                            text = { Text(tab.label) }
                        )
                    }
                }

                when (selectedTab) {
                    ResultInspectorTab.CREASES -> {
                        // Captured image + detected creases canvas.
                        BoxWithConstraints(
                            modifier = Modifier.weight(1f).fillMaxWidth().background(Color.Black)
                        ) {
                            val density = LocalDensity.current
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
                                        LocalizedStrings.tr("result.noPhoto", selectedLanguage),
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        style = MaterialTheme.typography.bodyMedium,
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

                        // Detected lines as native list items.
                        LazyColumn(modifier = Modifier.fillMaxWidth().height(220.dp)) {
                            item {
                                Text(
                                    text = LocalizedStrings.tr("result.detectedCreases", selectedLanguage)
                                        .format(splines.size),
                                    style = MaterialTheme.typography.titleSmall,
                                    color = MaterialTheme.colorScheme.onSurface,
                                    modifier = Modifier.fillMaxWidth()
                                        .background(MaterialTheme.colorScheme.surface)
                                        .padding(horizontal = 20.dp, vertical = 8.dp)
                                )
                            }
                            itemsIndexed(splines, key = { _, s -> s.id }) { index, spline ->
                                CreaseLineRow(
                                    spline = spline,
                                    selectedLanguage = selectedLanguage,
                                    onClick = {
                                        selectedSplineId = spline.id
                                        showEditor = true
                                    }
                                )
                                if (index < splines.lastIndex) {
                                    HorizontalDivider(modifier = Modifier.padding(horizontal = 20.dp))
                                }
                            }
                        }
                    }

                    ResultInspectorTab.TOPOGRAPHY -> {
                        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                            HandMeshInspectorView(
                                landmarks = landmarks,
                                padDepths = padDepths,
                                referencePalmWidthMm = 80.0f,
                                modifier = Modifier.fillMaxSize()
                            )
                        }
                    }

                    ResultInspectorTab.ANTHROPOMETRY -> {
                        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                            AnthropometrySummaryView(
                                landmarks = landmarks,
                                padDepths = padDepths,
                                referencePalmWidthMm = 80.0f,
                                handSide = handSide.name.lowercase(),
                                modifier = Modifier.fillMaxSize()
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
    ListItem(
        headlineContent = {
            Text(LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage))
        },
        supportingContent = {
            Row(
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("${spline.points.size} pts")
                if (spline.isUserAdjusted) {
                    Text(
                        text = "• " + LocalizedStrings.tr("result.adjusted", selectedLanguage),
                        color = MaterialTheme.colorScheme.primary
                    )
                }
            }
        },
        leadingContent = {
            Box(
                modifier = Modifier.size(width = 18.dp, height = 5.dp)
                    .clip(CircleShape)
                    .background(color)
            )
        },
        trailingContent = {
            Row(
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "${(spline.confidence * 100).toInt()}%",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary
                )
                Icon(
                    Icons.Filled.ChevronRight, null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(20.dp)
                )
            }
        },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        modifier = Modifier.clickable { onClick() }
    )
}

private fun parseResultHex(hex: String): Color {
    val c = hex.removePrefix("#").toLongOrNull(16) ?: 0xFFFFFF
    return Color(0xFF000000 or c)
}
