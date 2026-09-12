package org.curv.palmapp.view

import android.graphics.Bitmap
import android.graphics.PointF
import android.view.HapticFeedbackConstants
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Tag
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material.icons.filled.Undo
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.curv.palmapp.model.AppLanguage
import org.curv.palmapp.model.CreaseLineType
import org.curv.palmapp.model.CreaseSpline
import org.curv.palmapp.model.LocalizedStrings
import kotlin.math.hypot

/**
 * Dedicated full-screen precision editor for palm crease lines.
 * Phase 2b of Palm Scanner workflow — 1:1 port of iOS CreaseLineEditorView.swift:
 * - Each crease line is ONE continuous Catmull-Rom curve controlled by exactly
 *   4 points: Start (S), Curve Mid 1, Curve Mid 2, End (E). Zero clutter.
 * - Unified 5-tool bottom bar (icon above text caption) with equal spacing.
 * - Dynamic state-aware coaching pill: guides every interaction, auto-dismisses.
 * - Zoom & Pan (1.0x–4.0x) with 2.4x magnifying loupe while dragging a vertex.
 */
@Composable
fun CreaseLineEditorScreen(
    bitmap: Bitmap?,
    splines: List<CreaseSpline>,
    selectedLanguage: AppLanguage,
    onDismiss: () -> Unit,
    onDone: (List<CreaseSpline>) -> Unit,
    modifier: Modifier = Modifier
) {
    val view = LocalView.current
    val scope = rememberCoroutineScope()

    // Guarantee all lines are clean 4-point control splines (parity with iOS init).
    val cleanInitial = remember(splines) {
        splines.map { it.cleaned(4) }
    }
    var workingSplines by remember(cleanInitial) { mutableStateOf(cleanInitial) }
    val initialSplines = remember(cleanInitial) { cleanInitial }
    val history = remember { mutableStateListOf<List<CreaseSpline>>() }
    var selectedSplineId by remember(cleanInitial) { mutableStateOf(cleanInitial.firstOrNull()?.id) }
    var selectedVertexIndex by remember { mutableStateOf<Int?>(null) }

    var isDrawMode by remember { mutableStateOf(false) }
    val freehandCanvasPoints = remember { mutableStateListOf<Offset>() }

    // Dynamic coaching pill.
    var dynamicHint by remember { mutableStateOf("") }
    var showCoachingPill by remember { mutableStateOf(true) }
    var coachJob by remember { mutableStateOf<Job?>(null) }
    fun updateCoaching(text: String, autoDismissAfterSec: Double? = 3.5) {
        dynamicHint = text
        showCoachingPill = true
        coachJob?.cancel()
        if (autoDismissAfterSec != null) {
            coachJob = scope.launch {
                delay((autoDismissAfterSec * 1000).toLong())
                showCoachingPill = false
            }
        }
    }

    // Zoom & pan.
    var zoomScale by remember { mutableFloatStateOf(1f) }
    var panOffset by remember { mutableStateOf(Offset.Zero) }
    fun resetZoom() {
        zoomScale = 1f
        panOffset = Offset.Zero
    }

    // Drag state.
    var isDraggingVertex by remember { mutableStateOf(false) }

    val activeSpline = workingSplines.firstOrNull { it.id == selectedSplineId }

    fun pushHistory() {
        history.add(workingSplines)
        if (history.size > 20) history.removeAt(0)
    }
    fun moveVertex(splineId: String, index: Int, norm: Offset) {
        val i = workingSplines.indexOfFirst { it.id == splineId }
        if (i < 0 || index !in workingSplines[i].points.indices) return
        val pts = workingSplines[i].points.toMutableList()
        pts[index] = PointF(
            norm.x.coerceIn(0.01f, 0.99f),
            norm.y.coerceIn(0.01f, 0.99f)
        )
        workingSplines = workingSplines.toMutableList().also {
            it[i] = workingSplines[i].replacing(points = pts)
        }
    }
    fun finishFreehandDraw(drawRect: Rect) {
        if (freehandCanvasPoints.size < 4 || selectedSplineId == null) {
            freehandCanvasPoints.clear()
            return
        }
        val sid = selectedSplineId!!
        val sIndex = workingSplines.indexOfFirst { it.id == sid }
        if (sIndex < 0) {
            freehandCanvasPoints.clear()
            return
        }
        pushHistory()
        // 3-tap moving-average smoothing filter to eliminate finger digitizer micro-jitter
        val smoothedCanvas = smoothPoints(freehandCanvasPoints.toList())
        val normDrawn = smoothedCanvas.map { canvasPt ->
            val unmapped = ImageCoordinateMapper.unmap(canvasPt, drawRect)
            Offset(
                unmapped.x.coerceIn(0.01f, 0.99f),
                unmapped.y.coerceIn(0.01f, 0.99f)
            )
        }.map { PointF(it.x, it.y) }
        val four = CreaseSpline.resampleToControlPoints(normDrawn, 4)
        workingSplines = workingSplines.toMutableList().also {
            it[sIndex] = workingSplines[sIndex].replacing(points = four)
        }
        freehandCanvasPoints.clear()
        view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
        updateCoaching(LocalizedStrings.tr("editor.coachingCreated", selectedLanguage), 2.5)
    }

    LaunchedEffect(Unit) {
        updateCoaching(LocalizedStrings.tr("editor.hintAdjust", selectedLanguage), 4.0)
    }

    Column(modifier = modifier.fillMaxSize().background(Color.Black)) {
        // Native top app bar (status-bar insets handled by TopAppBar itself).
        EditorTopBar(
            selectedLanguage = selectedLanguage,
            activeSpline = activeSpline,
            onDismiss = onDismiss,
            onDone = {
                view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                onDone(workingSplines)
            }
        )

        // Main canvas area.
        BoxWithConstraints(
            modifier = Modifier.weight(1f).fillMaxWidth()
                .pointerInput(isDrawMode, isDraggingVertex) {
                    // Pinch-to-zoom (two fingers) + pan when zoomed.
                    detectTransformGestures { _, pan, zoom, _ ->
                        if (isDrawMode || isDraggingVertex) return@detectTransformGestures
                        val newScale = (zoomScale * zoom).coerceIn(1f, 4f)
                        zoomScale = newScale
                        if (newScale > 1.05f) {
                            panOffset = Offset(panOffset.x + pan.x, panOffset.y + pan.y)
                        } else {
                            panOffset = Offset.Zero
                        }
                    }
                }
        ) {
            val density = LocalDensity.current
            val viewSizePx = with(density) {
                Size(maxWidth.toPx(), maxHeight.toPx())
            }
            val imgSize = if (bitmap != null) Size(bitmap.width.toFloat(), bitmap.height.toFloat()) else viewSizePx
            val baseDrawRect = ImageCoordinateMapper.imageRect(imgSize, viewSizePx, ImageCoordinateMapper.ContentMode.FIT)
            val rectWdp = with(density) { baseDrawRect.width.toDp() }
            val rectHdp = with(density) { baseDrawRect.height.toDp() }
            val rectLeftDp = with(density) { baseDrawRect.left.toDp() }
            val rectTopDp = with(density) { baseDrawRect.top.toDp() }

            Box(
                modifier = Modifier.fillMaxSize()
                    .graphicsLayer(
                        scaleX = zoomScale, scaleY = zoomScale,
                        translationX = panOffset.x, translationY = panOffset.y
                    )
                    .pointerInput(workingSplines, selectedSplineId, isDrawMode, baseDrawRect) {
                        detectDragGestures(
                            onDragStart = { start ->
                                if (isDrawMode) {
                                    freehandCanvasPoints.clear()
                                    updateCoaching(
                                        LocalizedStrings.tr("editor.hintTracing", selectedLanguage),
                                        null
                                    )
                                } else {
                                    val spline = workingSplines.firstOrNull { it.id == selectedSplineId }
                                    if (spline != null) {
                                        var bestIdx = -1
                                        var bestDist = Float.MAX_VALUE
                                        for ((idx, pt) in spline.points.withIndex()) {
                                            val canvasPt = ImageCoordinateMapper.map(
                                                Offset(pt.x, pt.y), baseDrawRect
                                            )
                                            val d = hypot(canvasPt.x - start.x, canvasPt.y - start.y)
                                            if (d < bestDist) {
                                                bestDist = d
                                                bestIdx = idx
                                            }
                                        }
                                        if (bestDist < 68f * density.density && bestIdx >= 0) {
                                            pushHistory()
                                            isDraggingVertex = true
                                            selectedVertexIndex = bestIdx
                                            view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                                            val isStart = bestIdx == 0
                                            val isEnd = bestIdx == spline.points.size - 1
                                            updateCoaching(
                                                when {
                                                    isStart -> LocalizedStrings.tr("editor.hintMovingStart", selectedLanguage)
                                                    isEnd -> LocalizedStrings.tr("editor.hintMovingEnd", selectedLanguage)
                                                    else -> LocalizedStrings.tr("editor.hintMovingCurve", selectedLanguage)
                                                },
                                                null
                                            )
                                        }
                                    }
                                }
                            },
                            onDrag = { change, _ ->
                                if (isDrawMode) {
                                    freehandCanvasPoints.add(change.position)
                                } else if (isDraggingVertex) {
                                    val spline = workingSplines.firstOrNull { it.id == selectedSplineId }
                                    val vIdx = selectedVertexIndex
                                    if (spline != null && vIdx != null) {
                                        selectedSplineId = spline.id
                                        val unmapped = ImageCoordinateMapper.unmap(change.position, baseDrawRect)
                                        moveVertex(spline.id, vIdx, unmapped)
                                    }
                                }
                                change.consume()
                            },
                            onDragEnd = {
                                if (isDrawMode) {
                                    finishFreehandDraw(baseDrawRect)
                                } else if (isDraggingVertex) {
                                    isDraggingVertex = false
                                    view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                                    updateCoaching(LocalizedStrings.tr("editor.coachingUpdated", selectedLanguage), 2.0)
                                }
                            },
                            onDragCancel = {
                                if (isDrawMode) freehandCanvasPoints.clear()
                                isDraggingVertex = false
                            }
                        )
                    }
            ) {
                // Photo.
                if (bitmap != null) {
                    Image(
                        bitmap = bitmap.asImageBitmap(),
                        contentDescription = null,
                        modifier = Modifier.width(rectWdp).height(rectHdp)
                            .align(Alignment.TopStart)
                            .offset(x = rectLeftDp, y = rectTopDp),
                        contentScale = ContentScale.FillBounds
                    )
                }
                // Splines + freehand trace.
                EditorSplineCanvas(
                    splines = workingSplines,
                    selectedSplineId = selectedSplineId,
                    imageRect = baseDrawRect,
                    freehandPoints = freehandCanvasPoints.toList(),
                    activeColorHex = activeSpline?.lineType?.hexColor,
                    modifier = Modifier.fillMaxSize()
                )
                // Handles for the active spline in Adjust mode.
                // Mid points: 5dp pinpoint dot (resting).
                // Start / End points: 10dp small dot circle (resting, twice the mid dot).
                // Clicked / Dragged: Smoothly grows into 26dp hollow reticle (zero-occlusion, 100% transparent center).
                if (!isDrawMode && activeSpline != null) {
                    val spline = activeSpline
                    val dotColor = parseHex(spline.lineType.hexColor)
                    for ((index, pt) in spline.points.withIndex()) {
                        val canvasPt = ImageCoordinateMapper.map(Offset(pt.x, pt.y), baseDrawRect)
                        val xDp = with(density) { canvasPt.x.toDp() }
                        val yDp = with(density) { canvasPt.y.toDp() }
                        val isStart = index == 0
                        val isEnd = index == spline.points.size - 1
                        val isEndpoint = isStart || isEnd
                        val isSelected = index == selectedVertexIndex

                        Box(
                            modifier = Modifier.offset(x = xDp - 22.dp, y = yDp - 22.dp)
                                .size(44.dp),
                            contentAlignment = Alignment.Center
                        ) {
                            if (isSelected) {
                                // Active Expanded State: Precision Hollow Reticle (Zero-Occlusion)
                                Box(
                                    modifier = Modifier.size(34.dp)
                                        .border(1.5.dp, Color.White.copy(alpha = 0.85f), CircleShape)
                                )
                                Box(
                                    modifier = Modifier.size(26.dp)
                                        .border(3.5.dp, Color.Black.copy(alpha = 0.60f), CircleShape)
                                )
                                Box(
                                    modifier = Modifier.size(26.dp)
                                        .border(2.2.dp, dotColor, CircleShape)
                                )
                                Box(
                                    modifier = Modifier.size(21.dp)
                                        .border(0.8.dp, Color.White.copy(alpha = 0.85f), CircleShape)
                                )
                            } else if (isEndpoint) {
                                // Resting Endpoint (Start / End): Small Dot Circle (10dp, twice the mid dot)
                                Box(
                                    modifier = Modifier.size(10.dp)
                                        .border(2.5.dp, Color.Black.copy(alpha = 0.50f), CircleShape)
                                )
                                Box(
                                    modifier = Modifier.size(10.dp)
                                        .border(1.8.dp, dotColor, CircleShape)
                                )
                                Box(
                                    modifier = Modifier.size(3.5.dp)
                                        .background(Color.White, CircleShape)
                                )
                            } else {
                                // Resting Mid Points: Almost Dot Only (5dp pinpoint dot)
                                Box(
                                    modifier = Modifier.size(7.dp)
                                        .background(Color.Black.copy(alpha = 0.50f), CircleShape)
                                )
                                Box(
                                    modifier = Modifier.size(5.dp)
                                        .background(dotColor, CircleShape)
                                        .border(0.7.dp, Color.White.copy(alpha = 0.90f), CircleShape)
                                )
                            }
                        }
                    }
                }
            }

            // Floating reset-zoom button.
            if (zoomScale > 1.05f) {
                Row(modifier = Modifier.fillMaxWidth().padding(top = 16.dp, end = 16.dp)) {
                    Spacer(Modifier.weight(1f))
                    Text(
                        text = LocalizedStrings.tr("editor.resetZoom", selectedLanguage),
                        color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.Bold,
                        modifier = Modifier.clip(CircleShape)
                            .background(Color.White.copy(alpha = 0.15f))
                            .border(1.dp, Color.White.copy(alpha = 0.3f), CircleShape)
                            .clickable { resetZoom() }
                            .padding(horizontal = 10.dp, vertical = 6.dp)
                    )
                }
            }

            // Coaching pill (floating, auto-dismissing).
            if (showCoachingPill && dynamicHint.isNotEmpty()) {
                Row(modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                    Spacer(Modifier.weight(1f))
                    Row(
                        modifier = Modifier.clip(CircleShape)
                            .background(Color.Black.copy(alpha = 0.55f))
                            .border(0.8.dp, Color.White.copy(alpha = 0.2f), CircleShape)
                            .padding(horizontal = 14.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Icon(Icons.Filled.AutoAwesome, null, tint = Color(0xFF34C759), modifier = Modifier.size(14.dp))
                        Text(dynamicHint, color = Color.White.copy(alpha = 0.95f), fontSize = 12.sp)
                    }
                    Spacer(Modifier.weight(1f))
                }
            }
        }

        // Native line selector + bottom tool bar.
        EditorBottomStudio(
            workingSplines = workingSplines,
            selectedSplineId = selectedSplineId,
            selectedLanguage = selectedLanguage,
            isDrawMode = isDrawMode,
            canUndo = history.isNotEmpty(),
            onSelectSpline = { spline ->
                selectedSplineId = spline.id
                selectedVertexIndex = null
                view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                updateCoaching(
                    LocalizedStrings.tr("editor.coachingSelected", selectedLanguage)
                        .replace("{name}", LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage)),
                    2.0
                )
            },
            onAdjust = {
                isDrawMode = false
                view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                updateCoaching(LocalizedStrings.tr("editor.hintAdjust", selectedLanguage), 3.5)
            },
            onDraw = {
                isDrawMode = true
                view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                updateCoaching(LocalizedStrings.tr("editor.hintDraw", selectedLanguage), 4.0)
            },
            onUndo = {
                val prev = history.removeLastOrNull()
                if (prev != null) {
                    workingSplines = prev
                    view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                }
                updateCoaching(LocalizedStrings.tr("editor.coachingUndone", selectedLanguage), 2.0)
            },
            onReset = {
                val sid = selectedSplineId
                val sIndex = workingSplines.indexOfFirst { it.id == sid }
                val initial = initialSplines.firstOrNull { it.id == sid }
                if (sIndex >= 0 && initial != null) {
                    pushHistory()
                    workingSplines = workingSplines.toMutableList().also { it[sIndex] = initial }
                    selectedVertexIndex = null
                    view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                }
                updateCoaching(LocalizedStrings.tr("editor.coachingReset", selectedLanguage), 2.0)
            },
            onReassign = { type ->
                val sid = selectedSplineId
                val idx = workingSplines.indexOfFirst { it.id == sid }
                if (idx >= 0) {
                    pushHistory()
                    workingSplines = workingSplines.toMutableList().also {
                        it[idx] = workingSplines[idx].replacing(lineType = type)
                    }
                    view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                    updateCoaching(
                        LocalizedStrings.tr("editor.coachingReassigned", selectedLanguage)
                            .replace("{name}", LocalizedStrings.tr(type.displayNameKey, selectedLanguage)),
                        2.5
                    )
                }
            }
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun EditorTopBar(
    selectedLanguage: AppLanguage,
    activeSpline: CreaseSpline?,
    onDismiss: () -> Unit,
    onDone: () -> Unit
) {
    TopAppBar(
        title = {
            Column {
                Text(
                    text = LocalizedStrings.tr("editor.title", selectedLanguage),
                    style = MaterialTheme.typography.titleLarge
                )
                if (activeSpline != null) {
                    Text(
                        text = LocalizedStrings.tr(activeSpline.lineType.displayNameKey, selectedLanguage),
                        style = MaterialTheme.typography.bodySmall,
                        color = parseHex(activeSpline.lineType.hexColor)
                    )
                }
            }
        },
        navigationIcon = {
            TextButton(onClick = onDismiss) {
                Text(LocalizedStrings.tr("editor.cancel", selectedLanguage))
            }
        },
        actions = {
            Button(onClick = onDone) {
                Icon(Icons.Filled.Check, null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.size(4.dp))
                Text(LocalizedStrings.tr("editor.done", selectedLanguage))
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = Color.Black,
            titleContentColor = Color.White,
            navigationIconContentColor = Color.White,
            actionIconContentColor = Color.Black
        )
    )
}

/** Native segmented line selector + bottom navigation-bar tools. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun EditorBottomStudio(
    workingSplines: List<CreaseSpline>,
    selectedSplineId: String?,
    selectedLanguage: AppLanguage,
    isDrawMode: Boolean,
    canUndo: Boolean,
    onSelectSpline: (CreaseSpline) -> Unit,
    onAdjust: () -> Unit,
    onDraw: () -> Unit,
    onUndo: () -> Unit,
    onReset: () -> Unit,
    onReassign: (CreaseLineType) -> Unit
) {
    var labelMenu by remember { mutableStateOf(false) }

    Column(modifier = Modifier.fillMaxWidth()) {
        // Line selector: native single-choice segmented buttons.
        SingleChoiceSegmentedButtonRow(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)
        ) {
            workingSplines.forEachIndexed { index, spline ->
                SegmentedButton(
                    selected = spline.id == selectedSplineId,
                    onClick = { onSelectSpline(spline) },
                    shape = SegmentedButtonDefaults.itemShape(index, workingSplines.size),
                    icon = {
                        SegmentedButtonDefaults.Icon(active = spline.id == selectedSplineId) {
                            Box(
                                Modifier.size(8.dp).clip(CircleShape)
                                    .background(parseHex(spline.lineType.hexColor))
                            )
                        }
                    },
                    label = {
                        Text(
                            text = LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage),
                            maxLines = 1
                        )
                    }
                )
            }
        }

        // Five tools as a native bottom navigation bar.
        Box(modifier = Modifier.fillMaxWidth()) {
            NavigationBar(containerColor = Color.Transparent) {
                NavigationBarItem(
                    selected = !isDrawMode,
                    onClick = onAdjust,
                    icon = { Icon(Icons.Filled.Tune, null) },
                    label = { Text(LocalizedStrings.tr("editor.toolAdjust", selectedLanguage)) }
                )
                NavigationBarItem(
                    selected = isDrawMode,
                    onClick = onDraw,
                    icon = { Icon(Icons.Filled.Edit, null) },
                    label = { Text(LocalizedStrings.tr("editor.toolDraw", selectedLanguage)) }
                )
                NavigationBarItem(
                    selected = false,
                    enabled = canUndo,
                    onClick = onUndo,
                    icon = { Icon(Icons.Filled.Undo, null) },
                    label = { Text(LocalizedStrings.tr("editor.toolUndo", selectedLanguage)) }
                )
                NavigationBarItem(
                    selected = false,
                    onClick = onReset,
                    icon = { Icon(Icons.Filled.Refresh, null) },
                    label = { Text(LocalizedStrings.tr("editor.toolReset", selectedLanguage)) }
                )
                NavigationBarItem(
                    selected = false,
                    onClick = { labelMenu = true },
                    icon = { Icon(Icons.Filled.Tag, null) },
                    label = { Text(LocalizedStrings.tr("editor.toolLabel", selectedLanguage)) }
                )
            }
            // Relabel menu anchored to the bottom bar (native dropdown).
            DropdownMenu(expanded = labelMenu, onDismissRequest = { labelMenu = false }) {
                CreaseLineType.values().forEach { type ->
                    DropdownMenuItem(
                        text = { Text(LocalizedStrings.tr(type.displayNameKey, selectedLanguage)) },
                        onClick = {
                            onReassign(type)
                            labelMenu = false
                        }
                    )
                }
            }
        }
    }
}

private fun parseHex(hex: String): Color {
    val c = hex.removePrefix("#").toLongOrNull(16) ?: 0xFFFFFF
    return Color(0xFF000000 or c)
}

/** Draws all crease splines as smooth Catmull-Rom curves + in-progress freehand trace. */
@Composable
private fun EditorSplineCanvas(
    splines: List<CreaseSpline>,
    selectedSplineId: String?,
    imageRect: Rect,
    freehandPoints: List<Offset>,
    activeColorHex: String?,
    modifier: Modifier = Modifier
) {
    androidx.compose.foundation.Canvas(modifier = modifier.fillMaxSize()) {
        for (spline in splines) {
            if (spline.points.size < 2) continue
            val canvasPoints = spline.points.map { pt ->
                ImageCoordinateMapper.map(Offset(pt.x, pt.y), imageRect)
            }
            val path = buildEditorCatmullRom(canvasPoints)
            val color = parseHex(spline.lineType.hexColor)
            val isSelected = spline.id == selectedSplineId
            // Outer soft glow.
            drawPath(
                path = path,
                color = color.copy(alpha = if (isSelected) 0.38f else 0.12f),
                style = androidx.compose.ui.graphics.drawscope.Stroke(
                    width = if (isSelected) 10f else 5f,
                    cap = androidx.compose.ui.graphics.StrokeCap.Round,
                    join = androidx.compose.ui.graphics.StrokeJoin.Round
                )
            )
            // Solid continuous curve.
            drawPath(
                path = path,
                color = color.copy(alpha = if (isSelected) 1f else 0.35f),
                style = androidx.compose.ui.graphics.drawscope.Stroke(
                    width = if (isSelected) 4f else 2.5f,
                    cap = androidx.compose.ui.graphics.StrokeCap.Round,
                    join = androidx.compose.ui.graphics.StrokeJoin.Round
                )
            )
        }
        // In-progress freehand trace stroke.
        if (freehandPoints.isNotEmpty()) {
            val trace = androidx.compose.ui.graphics.Path().apply {
                moveTo(freehandPoints[0].x, freehandPoints[0].y)
                for (pt in freehandPoints.drop(1)) lineTo(pt.x, pt.y)
            }
            val active = activeColorHex?.let { parseHex(it) } ?: Color.White
            drawPath(
                path = trace, color = active,
                style = androidx.compose.ui.graphics.drawscope.Stroke(
                    width = 4f,
                    cap = androidx.compose.ui.graphics.StrokeCap.Round,
                    join = androidx.compose.ui.graphics.StrokeJoin.Round
                )
            )
        }
    }
}

private fun buildEditorCatmullRom(points: List<Offset>): androidx.compose.ui.graphics.Path {
    val path = androidx.compose.ui.graphics.Path()
    if (points.size < 2) return path
    path.moveTo(points[0].x, points[0].y)
    if (points.size == 2) {
        path.lineTo(points[1].x, points[1].y)
        return path
    }
    for (i in 0 until points.size - 1) {
        val p0 = if (i > 0) points[i - 1] else points[i]
        val p1 = points[i]
        val p2 = points[i + 1]
        val p3 = if (i + 2 < points.size) points[i + 2] else p2
        val cp1x = p1.x + (p2.x - p0.x) / 6f
        val cp1y = p1.y + (p2.y - p0.y) / 6f
        val cp2x = p2.x - (p3.x - p1.x) / 6f
        val cp2y = p2.y - (p3.y - p1.y) / 6f
        path.cubicTo(cp1x, cp1y, cp2x, cp2y, p2.x, p2.y)
    }
    return path
}

/** 3-tap moving-average smoothing filter to eliminate finger digitizer micro-jitter (parity with iOS). */
private fun smoothPoints(pts: List<Offset>): List<Offset> {
    if (pts.size <= 3) return pts
    val result = mutableListOf<Offset>()
    result.add(pts.first())
    for (i in 1 until pts.size - 1) {
        val prev = pts[i - 1]
        val curr = pts[i]
        val next = pts[i + 1]
        val sx = (prev.x * 0.25f) + (curr.x * 0.50f) + (next.x * 0.25f)
        val sy = (prev.y * 0.25f) + (curr.y * 0.50f) + (next.y * 0.25f)
        result.add(Offset(sx, sy))
    }
    result.add(pts.last())
    return result
}
