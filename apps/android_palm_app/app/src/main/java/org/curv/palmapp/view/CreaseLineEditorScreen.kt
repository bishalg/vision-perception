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
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Tag
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material.icons.filled.Undo
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
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
    var selectedVertexIndex by remember { mutableStateOf<Int?>(0) }

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

    // Loupe state.
    var isDraggingVertex by remember { mutableStateOf(false) }
    var loupeLocation by remember { mutableStateOf(Offset.Zero) }
    var loupeNormPoint by remember { mutableStateOf(Offset.Zero) }

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
        val normDrawn = freehandCanvasPoints.map { canvasPt ->
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
        updateCoaching("New curve created", 2.5)
    }

    LaunchedEffect(Unit) {
        updateCoaching(LocalizedStrings.tr("editor.hintAdjust", selectedLanguage), 4.0)
    }

    Column(modifier = modifier.fillMaxSize().background(Color.Black)) {
        // Top navigation bar.
        Row(
            modifier = Modifier.fillMaxWidth().padding(top = 50.dp, start = 16.dp, end = 16.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = LocalizedStrings.tr("editor.cancel", selectedLanguage),
                color = Color.White.copy(alpha = 0.85f),
                fontSize = 15.sp,
                modifier = Modifier.clip(CircleShape).background(Color.White.copy(alpha = 0.12f))
                    .clickable { onDismiss() }.padding(horizontal = 12.dp, vertical = 7.dp)
            )
            Spacer(Modifier.weight(1f))
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    text = LocalizedStrings.tr("editor.title", selectedLanguage),
                    color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold
                )
                if (activeSpline != null) {
                    Text(
                        text = LocalizedStrings.tr(activeSpline.lineType.displayNameKey, selectedLanguage),
                        color = parseHex(activeSpline.lineType.hexColor),
                        fontSize = 12.sp, fontWeight = FontWeight.SemiBold
                    )
                }
            }
            Spacer(Modifier.weight(1f))
            Row(
                modifier = Modifier.clip(CircleShape).background(Color(0xFF34C759))
                    .clickable {
                        view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                        onDone(workingSplines)
                    }.padding(horizontal = 16.dp, vertical = 7.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp)
            ) {
                Icon(Icons.Default.Check, null, tint = Color.Black, modifier = Modifier.size(14.dp))
                Text(
                    text = LocalizedStrings.tr("editor.done", selectedLanguage),
                    color = Color.Black, fontSize = 15.sp, fontWeight = FontWeight.Bold
                )
            }
        }

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
                                        loupeLocation = change.position
                                        loupeNormPoint = unmapped
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
                                    updateCoaching("Curve updated", 2.0)
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
                if (!isDrawMode && activeSpline != null) {
                    val spline = activeSpline
                    val dotColor = parseHex(spline.lineType.hexColor)
                    for ((index, pt) in spline.points.withIndex()) {
                        val canvasPt = ImageCoordinateMapper.map(Offset(pt.x, pt.y), baseDrawRect)
                        val canvasDp = with(density) {
                            Offset(canvasPt.x.toDp().value, canvasPt.y.toDp().value)
                        }
                        // Positions are in px; convert via density-aware offset below.
                        val xDp = with(density) { canvasPt.x.toDp() }
                        val yDp = with(density) { canvasPt.y.toDp() }
                        val isStart = index == 0
                        val isSelected = index == selectedVertexIndex
                        val ringDiameter: Dp = 22.dp
                        Box(
                            modifier = Modifier.offset(x = xDp - 22.dp, y = yDp - 22.dp)
                                .size(44.dp),
                            contentAlignment = Alignment.Center
                        ) {
                            // Selection subtle halo
                            if (isSelected) {
                                Box(
                                    modifier = Modifier.size(ringDiameter + 8.dp)
                                        .border(1.5.dp, Color.White.copy(alpha = 0.85f), CircleShape)
                                )
                            }
                            // Outer dark shadow ring for high contrast
                            Box(
                                modifier = Modifier.size(ringDiameter)
                                    .border(3.dp, Color.Black.copy(alpha = 0.50f), CircleShape)
                            )
                            // Precision hollow ring: stroke only, 100% transparent center!
                            Box(
                                modifier = Modifier.size(ringDiameter)
                                    .border(2.dp, dotColor, CircleShape)
                            )
                            // Inner delicate guide ring
                            Box(
                                modifier = Modifier.size(ringDiameter - 4.dp)
                                    .border(0.8.dp, Color.White.copy(alpha = 0.8f), CircleShape)
                            )
                        }
                        // Silence unused warning for canvasDp parity computations.
                        @Suppress("UNUSED_EXPRESSION")
                        canvasDp
                    }
                }
            }

            // Floating 2.4x magnifying loupe while dragging.
            if (isDraggingVertex && bitmap != null) {
                val density2 = LocalDensity.current
                val loupeDp = 104.dp
                val lx = with(density2) { loupeLocation.x.toDp() } * 1f
                val ly = with(density2) { loupeLocation.y.toDp() }
                // Position above the finger; coordinates here are unscaled canvas px.
                Box(
                    modifier = Modifier.offset(
                        x = (lx - loupeDp / 2),
                        y = (ly - 85.dp - loupeDp / 2).coerceAtLeast(8.dp)
                    ).size(loupeDp)
                ) {
                    MagnifyingLoupe(
                        bitmap = bitmap,
                        focusNorm = loupeNormPoint,
                        lineColor = activeSpline?.let { parseHex(it.lineType.hexColor) } ?: Color.White,
                        diameter = loupeDp
                    )
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
                        Text("✦", color = Color(0xFF34C759), fontSize = 11.sp, fontWeight = FontWeight.Bold)
                        Text(dynamicHint, color = Color.White.copy(alpha = 0.95f), fontSize = 12.sp)
                    }
                    Spacer(Modifier.weight(1f))
                }
            }
        }

        // Bottom floating control studio.
        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 14.dp)
                .padding(top = 12.dp, bottom = 22.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            // Line selection tabs.
            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(workingSplines, key = { it.id }) { spline ->
                    val isSelected = spline.id == selectedSplineId
                    val color = parseHex(spline.lineType.hexColor)
                    Row(
                        modifier = Modifier.clip(CircleShape)
                            .background(if (isSelected) color.copy(alpha = 0.28f) else Color.White.copy(alpha = 0.08f))
                            .border(
                                1.dp,
                                if (isSelected) color else Color.White.copy(alpha = 0.15f),
                                CircleShape
                            )
                            .clickable {
                                selectedSplineId = spline.id
                                selectedVertexIndex = 0
                                view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                                updateCoaching(
                                    "Selected ${LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage)}",
                                    2.0
                                )
                            }
                            .padding(horizontal = 14.dp, vertical = 7.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Box(Modifier.size(8.dp).clip(CircleShape).background(color))
                        Text(
                            text = LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage),
                            color = if (isSelected) Color.White else Color.White.copy(alpha = 0.65f),
                            fontSize = 13.sp,
                            fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Medium
                        )
                    }
                }
            }

            // 5-tool equal row: Adjust | Draw | Undo | Reset | Label.
            Row(modifier = Modifier.fillMaxWidth()) {
                EditorToolButton(
                    icon = "Adjust",
                    title = LocalizedStrings.tr("editor.toolAdjust", selectedLanguage),
                    isActive = !isDrawMode,
                    modifier = Modifier.weight(1f)
                ) {
                    isDrawMode = false
                    view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                    updateCoaching(LocalizedStrings.tr("editor.hintAdjust", selectedLanguage), 3.5)
                }
                EditorToolButton(
                    icon = "Draw",
                    title = LocalizedStrings.tr("editor.toolDraw", selectedLanguage),
                    isActive = isDrawMode,
                    modifier = Modifier.weight(1f)
                ) {
                    isDrawMode = true
                    view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                    updateCoaching(LocalizedStrings.tr("editor.hintDraw", selectedLanguage), 4.0)
                }
                EditorToolButton(
                    icon = "Undo",
                    title = LocalizedStrings.tr("editor.toolUndo", selectedLanguage),
                    isDisabled = history.isEmpty(),
                    modifier = Modifier.weight(1f)
                ) {
                    val prev = history.removeLastOrNull()
                    if (prev != null) {
                        workingSplines = prev
                        view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                    }
                    updateCoaching("Action undone", 2.0)
                }
                EditorToolButton(
                    icon = "Reset",
                    title = LocalizedStrings.tr("editor.toolReset", selectedLanguage),
                    modifier = Modifier.weight(1f)
                ) {
                    val sid = selectedSplineId
                    val sIndex = workingSplines.indexOfFirst { it.id == sid }
                    val initial = initialSplines.firstOrNull { it.id == sid }
                    if (sIndex >= 0 && initial != null) {
                        pushHistory()
                        workingSplines = workingSplines.toMutableList().also { it[sIndex] = initial }
                        selectedVertexIndex = 0
                        view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                    }
                    updateCoaching("Line reset to original", 2.0)
                }
                // Label tool (reassign line type).
                var labelMenu by remember { mutableStateOf(false) }
                Box(modifier = Modifier.weight(1f)) {
                    Column(
                        modifier = Modifier.fillMaxWidth().clickable { labelMenu = true }
                            .padding(vertical = 6.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        Icon(Icons.Default.Tag, null, tint = Color.White.copy(alpha = 0.85f), modifier = Modifier.size(18.dp))
                        Text(
                            text = LocalizedStrings.tr("editor.toolLabel", selectedLanguage),
                            color = Color.White.copy(alpha = 0.85f), fontSize = 10.sp
                        )
                    }
                    DropdownMenu(expanded = labelMenu, onDismissRequest = { labelMenu = false }) {
                        CreaseLineType.values().forEach { type ->
                            DropdownMenuItem(
                                text = { Text(LocalizedStrings.tr(type.displayNameKey, selectedLanguage)) },
                                onClick = {
                                    val sid = selectedSplineId
                                    val idx = workingSplines.indexOfFirst { it.id == sid }
                                    if (idx >= 0) {
                                        pushHistory()
                                        workingSplines = workingSplines.toMutableList().also {
                                            it[idx] = workingSplines[idx].replacing(lineType = type)
                                        }
                                        view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                                        updateCoaching(
                                            "Reassigned to ${LocalizedStrings.tr(type.displayNameKey, selectedLanguage)}",
                                            2.5
                                        )
                                    }
                                    labelMenu = false
                                }
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun EditorToolButton(
    icon: String,
    title: String,
    isActive: Boolean = false,
    isDisabled: Boolean = false,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Column(
        modifier = modifier.clickable(enabled = !isDisabled) { onClick() }
            .padding(vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        val imageVector = when (icon) {
            "Draw" -> Icons.Default.Tune
            "Undo" -> Icons.Default.Undo
            "Reset" -> Icons.Default.Refresh
            else -> Icons.Default.Tune
        }
        Icon(
            imageVector = imageVector,
            contentDescription = null,
            tint = when {
                isDisabled -> Color.White.copy(alpha = 0.25f)
                isActive -> Color(0xFF34C759)
                else -> Color.White.copy(alpha = 0.85f)
            },
            modifier = Modifier.size(18.dp)
        )
        Text(
            text = title,
            color = when {
                isDisabled -> Color.White.copy(alpha = 0.25f)
                isActive -> Color(0xFF34C759)
                else -> Color.White.copy(alpha = 0.85f)
            },
            fontSize = 10.sp,
            fontWeight = if (isActive) FontWeight.Bold else FontWeight.Medium
        )
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

/**
 * Floating 2.4x circular magnifying loupe over the touch point.
 * Port of iOS MagnifyingLoupeView (104dp diameter).
 */
@Composable
private fun MagnifyingLoupe(
    bitmap: Bitmap,
    focusNorm: Offset,
    lineColor: Color,
    diameter: Dp,
    modifier: Modifier = Modifier
) {
    val imageBitmap = remember(bitmap) { bitmap.asImageBitmap() }
    val zoomFactor = 2.4f
    Box(
        modifier = modifier.size(diameter).clip(CircleShape)
            .background(Color.Black)
            .border(2.5.dp, lineColor, CircleShape),
        contentAlignment = Alignment.Center
    ) {
        androidx.compose.foundation.Canvas(modifier = Modifier.fillMaxSize().clip(CircleShape)) {
            val imgW = imageBitmap.width.toFloat()
            val imgH = imageBitmap.height.toFloat()
            val focusX = focusNorm.x.coerceIn(0f, 1f) * imgW
            val focusY = focusNorm.y.coerceIn(0f, 1f) * imgH
            val dstSize = size
            // Source window in bitmap px that fills the loupe at 2.4x.
            // dstSize is in canvas px; bitmap px per canvas px:
            val pxPerCanvasX = imgW / size.width.coerceAtLeast(1f)
            val pxPerCanvasY = imgH / size.height.coerceAtLeast(1f)
            val srcW = (dstSize.width / zoomFactor) * pxPerCanvasX
            val srcH = (dstSize.height / zoomFactor) * pxPerCanvasY
            val srcLeft = (focusX - srcW / 2f).coerceIn(0f, (imgW - srcW).coerceAtLeast(0f))
            val srcTop = (focusY - srcH / 2f).coerceIn(0f, (imgH - srcH).coerceAtLeast(0f))
            drawImage(
                image = imageBitmap,
                srcOffset = androidx.compose.ui.unit.IntOffset(srcLeft.toInt(), srcTop.toInt()),
                srcSize = androidx.compose.ui.unit.IntSize(srcW.toInt().coerceAtLeast(1), srcH.toInt().coerceAtLeast(1)),
                dstOffset = androidx.compose.ui.unit.IntOffset.Zero,
                dstSize = androidx.compose.ui.unit.IntSize(dstSize.width.toInt(), dstSize.height.toInt())
            )
        }
        Box(
            Modifier.size(8.dp).clip(CircleShape).background(lineColor)
                .border(1.5.dp, Color.White, CircleShape)
        )
    }
}
