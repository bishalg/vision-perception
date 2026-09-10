package org.curv.palmapp.view

import android.graphics.Bitmap
import android.graphics.PointF
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AddCircle
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.curv.palmapp.model.AppLanguage
import org.curv.palmapp.model.CreaseLineType
import org.curv.palmapp.model.CreaseOverlaySource
import org.curv.palmapp.model.CreaseSpline
import org.curv.palmapp.model.LocalizedStrings
import kotlin.math.hypot

/**
 * Full-screen post-capture review — v2: no language switch, [small Retake][large Use] bar,
 * debug analysis panel, and fixed drag (locks vertex onStart).
 */
@Composable
fun CaptureResultView(
    bitmap: Bitmap?,
    landmarks: List<PointF>,
    splines: List<CreaseSpline>,
    selectedLanguage: AppLanguage,
    overlaySource: CreaseOverlaySource,
    onSplinesChange: (List<CreaseSpline>) -> Unit,
    onLanguageChange: (AppLanguage) -> Unit = {},
    onRetake: () -> Unit,
    onConfirmAdjustedLines: () -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier
) {
    var isEditing by remember(overlaySource) { mutableStateOf(overlaySource == CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT) }
    var selectedId by remember(splines) { mutableStateOf(splines.firstOrNull()?.id) }
    var showDragHint by remember { mutableStateOf(true) }
    var showDebug by remember { mutableStateOf(false) }

    val needsManual = overlaySource == CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT
    val sourceSubtitle = if (needsManual) LocalizedStrings.tr("result.landmark", selectedLanguage)
    else LocalizedStrings.tr("result.native", selectedLanguage)
    val selectedSpline = splines.firstOrNull { it.id == selectedId }

    fun moveVertex(id: String, idx: Int, norm: Offset) {
        val list = splines.toMutableList()
        val i = list.indexOfFirst { it.id == id }
        if (i < 0 || idx !in list[i].points.indices) return
        val pts = list[i].points.toMutableList()
        pts[idx] = PointF(norm.x.coerceIn(0.02f, 0.98f), norm.y.coerceIn(0.02f, 0.98f))
        list[i] = list[i].replacing(points = pts)
        onSplinesChange(list); showDragHint = false
    }
    fun addPoint(id: String) {
        val list = splines.toMutableList()
        val i = list.indexOfFirst { it.id == id } ; if (i < 0) return
        val pts = list[i].points.toMutableList()
        if (pts.size < 2) pts.add(PointF(0.5f, 0.5f))
        else { var best=0; var bestLen=-1f; for(s in 0 until pts.size-1){ val len=hypot((pts[s+1].x-pts[s].x).toDouble(), (pts[s+1].y-pts[s].y).toDouble()).toFloat(); if(len>bestLen){bestLen=len; best=s}}; pts.add(best+1, PointF((pts[best].x+pts[best+1].x)*0.5f,(pts[best].y+pts[best+1].y)*0.5f)) }
        list[i]=list[i].replacing(points=pts); onSplinesChange(list)
    }
    fun removePoint(id: String) {
        val list=splines.toMutableList(); val i=list.indexOfFirst{it.id==id}; if(i<0)return; val pts=list[i].points.toMutableList(); if(pts.size<=2)return; pts.removeLast(); list[i]=list[i].replacing(points=pts); onSplinesChange(list)
    }
    fun reassign(id: String, to: CreaseLineType) {
        val list=splines.toMutableList(); val i=list.indexOfFirst{it.id==id}; if(i<0)return; val j=list.indexOfFirst{it.lineType==to && it.id!=id}; if(j>=0){val old=list[i].lineType; list[i]=list[i].replacing(lineType=to); list[j]=list[j].replacing(lineType=old)} else list[i]=list[i].replacing(lineType=to); onSplinesChange(list)
    }

    Column(modifier = modifier.fillMaxSize().background(Color.Black)) {
        BoxWithConstraints(modifier = Modifier.weight(1f).fillMaxWidth().background(Color.Black)) {
            val density = LocalDensity.current
            val viewSize = Size(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat())
            val imgSize = if (bitmap != null) Size(bitmap.width.toFloat(), bitmap.height.toFloat()) else viewSize
            val drawRect = ImageCoordinateMapper.imageRect(imgSize, viewSize, ImageCoordinateMapper.ContentMode.FIT)
            val rectWdp = with(density){ drawRect.width.toDp() }
            val rectHdp = with(density){ drawRect.height.toDp() }
            Box(modifier = Modifier.fillMaxSize()) {
                if (bitmap != null) {
                    Image(bitmap = bitmap.asImageBitmap(), contentDescription = null,
                        modifier = Modifier.width(rectWdp).height(rectHdp).align(Alignment.Center), contentScale = ContentScale.FillBounds)
                }
                HandOutlineOverlay(landmarks = landmarks, imageRect = drawRect, modifier = Modifier.fillMaxSize())
                CreaseSplineOverlayView(
                    splines = splines, imageRect = drawRect, selectedSplineId = selectedId, isEditing = isEditing, useScaffoldStyle = needsManual,
                    onSelect = { selectedId = it; if (!isEditing) isEditing = true },
                    onMoveVertex = { id, idx, norm -> moveVertex(id, idx, norm) },
                    modifier = Modifier.fillMaxSize()
                )
                if (needsManual && showDragHint && isEditing) {
                    DragHintOverlay(modifier = Modifier.align(Alignment.TopCenter).padding(top=12.dp))
                }
                Box(modifier = Modifier.padding(16.dp).size(36.dp).clip(CircleShape).background(Color.Black.copy(alpha=0.5f)).clickable{ onDismiss() }.align(Alignment.TopStart), contentAlignment=Alignment.Center){ Text("×", color=Color.White, fontSize=22.sp) }
            }
        }
        Column(modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(topStart=24.dp, topEnd=24.dp)).background(Color(0xFF1C1C1E)).padding(horizontal=20.dp, vertical=14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            if (needsManual) {
                Row(modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(Color(0xFF7F1D1D).copy(alpha=0.92f)).border(2.dp, Color(0xFFFBBF24).copy(alpha=0.7f), RoundedCornerShape(14.dp)).padding(14.dp), verticalAlignment=Alignment.Top, horizontalArrangement=Arrangement.spacedBy(10.dp)){
                    Icon(Icons.Default.Warning, null, tint=Color(0xFFFBBF24), modifier=Modifier.size(28.dp))
                    Column(verticalArrangement=Arrangement.spacedBy(4.dp)){ Text(LocalizedStrings.tr("result.manualBannerTitle", selectedLanguage), color=Color.White, fontWeight=FontWeight.Heavy, fontSize=16.sp); Text(LocalizedStrings.tr("result.manualBannerBody", selectedLanguage), color=Color.White.copy(0.85f), fontSize=12.sp) }
                }
            }
            Row(modifier=Modifier.fillMaxWidth(), horizontalArrangement=Arrangement.SpaceBetween, verticalAlignment=Alignment.Top){
                Column{ Text(LocalizedStrings.tr("result.title", selectedLanguage), color=Color.White, fontWeight=FontWeight.Bold, fontSize=18.sp); Text(sourceSubtitle, color=Color.White.copy(0.55f), fontSize=12.sp) }
            }
            Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){
                ChipButton(label=LocalizedStrings.tr(if(isEditing)"result.doneEdit" else "result.edit", selectedLanguage), icon=if(isEditing) Icons.Default.Check else Icons.Default.Tune, emphasized=isEditing, onClick={ isEditing=!isEditing; if(isEditing && selectedId==null) selectedId=splines.firstOrNull()?.id })
                if(isEditing && selectedId!=null){ ChipButton(LocalizedStrings.tr("result.addPoint", selectedLanguage), Icons.Default.AddCircle, false){ addPoint(selectedId!!) }
                    ChipButton("Remove", Icons.Default.Warning, false){ removePoint(selectedId!!) } }
                Spacer(Modifier.weight(1f))
                if(isEditing) TextButton(onClick={ val sid=selectedId; if(sid!=null){ val idx=splines.indexOfFirst{it.id==sid}; if(idx>=0){ val list=splines.toMutableList(); list[idx]=list[idx].replacing(points=list[idx].points, markAdjusted=false).copy(isUserAdjusted=false); onSplinesChange(list) } } }){ Text("Reset", color=Color.White.copy(0.75f), fontSize=11.sp) }
            }
            if(isEditing) Text(LocalizedStrings.tr("result.editHint", selectedLanguage)+" • Tap line to select, drag white dots.", color=Color.White.copy(0.62f), fontSize=11.sp)
            if(selectedSpline!=null){
                val c=parseHex(selectedSpline.lineType.hexColor)
                Row(modifier=Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(c.copy(alpha=0.18f)).border(1.5.dp,c.copy(0.55f),RoundedCornerShape(12.dp)).padding(10.dp), verticalAlignment=Alignment.CenterVertically, horizontalArrangement=Arrangement.spacedBy(8.dp)){
                    Box(Modifier.size(18.dp).clip(CircleShape).background(c)); Text(LocalizedStrings.tr(selectedSpline.lineType.displayNameKey, selectedLanguage), color=Color.White, fontWeight=FontWeight.Bold, fontSize=13.sp); Text("${selectedSpline.points.size} pts", color=Color.White.copy(0.6f), fontSize=11.sp)
                    if(selectedSpline.isUserAdjusted) Text("edited", color=Color(0xFF34C759), fontSize=10.sp, fontWeight=FontWeight.Bold, modifier=Modifier.clip(CircleShape).background(Color(0xFF34C759).copy(alpha=0.18f)).padding(horizontal=6.dp, vertical=2.dp))
                    Spacer(Modifier.weight(1f))
                    var expand by remember{ mutableStateOf(false) }
                    Box{ Icon(Icons.Default.Warning, null, tint=Color.White.copy(0.85f), modifier=Modifier.size(18.dp).clickable{expand=true}); DropdownMenu(expanded=expand, onDismissRequest={expand=false}){ CreaseLineType.values().forEach{t-> DropdownMenuItem(text={Text(LocalizedStrings.tr(t.displayNameKey, selectedLanguage))}, onClick={reassign(selectedSpline.id,t); expand=false})} } }
                }
            }
            Column(verticalArrangement=Arrangement.spacedBy(6.dp), modifier=Modifier.heightIn(max=220.dp)){
                splines.forEach{ spline ->
                    val sel=spline.id==selectedId; val col=parseHex(spline.lineType.hexColor)
                    Row(modifier=Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(if(sel) col.copy(0.14f) else Color.White.copy(alpha=0.04f)).border(if(sel)1.2.dp else 0.dp, if(sel)col.copy(0.55f) else Color.Transparent, RoundedCornerShape(10.dp)).clickable{ selectedId=spline.id; if(!isEditing){isEditing=true} }.padding(horizontal=8.dp, vertical=10.dp), verticalAlignment=Alignment.CenterVertically, horizontalArrangement=Arrangement.spacedBy(8.dp)){
                        Box(Modifier.size(if(sel)22.dp else 18.dp,4.dp).clip(CircleShape).background(col)); Text(LocalizedStrings.tr(spline.lineType.displayNameKey, selectedLanguage), color=Color.White, fontWeight=if(sel) FontWeight.Bold else FontWeight.SemiBold, fontSize=13.sp)
                        if(spline.isUserAdjusted) Text("• edited", color=Color(0xFF34C759), fontSize=11.sp, fontWeight=FontWeight.Bold)
                        Spacer(Modifier.weight(1f)); Text("${(spline.confidence*100).toInt()}%", color=Color(0xFF34C759), fontWeight=FontWeight.Bold, fontSize=11.sp, modifier=Modifier.clip(CircleShape).background(Color(0xFF34C759).copy(alpha=0.14f)).padding(horizontal=6.dp, vertical=2.dp))
                    }
                }
            }
            if(showDebug && splines.isNotEmpty()){
                Column(modifier=Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Color.Black.copy(0.28f)).border(1.dp, Color.White.copy(0.10f), RoundedCornerShape(12.dp)).padding(10.dp), verticalArrangement=Arrangement.spacedBy(8.dp)){
                    Row{ Text("Debug analysis", color=Color.White.copy(0.85f), fontWeight=FontWeight.Bold, fontSize=12.sp, fontFamily=androidx.compose.ui.text.font.FontFamily.Monospace); Spacer(); TextButton(onClick={showDebug=false}){ Text("Hide", color=Color.White.copy(0.6f), fontSize=11.sp) } }
                    splines.forEach{ s ->
                        val start=s.points.firstOrNull(); val end=s.points.lastOrNull()
                        Column(modifier=Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(Color.White.copy(alpha=0.06f)).padding(8.dp), verticalArrangement=Arrangement.spacedBy(2.dp)){
                            Row(horizontalArrangement=Arrangement.spacedBy(6.dp), verticalAlignment=Alignment.CenterVertically){
                                Box(Modifier.size(7.dp).clip(CircleShape).background(parseHex(s.lineType.hexColor))); Text(s.lineType.name, color=Color.White, fontWeight=FontWeight.Bold, fontSize=11.sp, fontFamily=androidx.compose.ui.text.font.FontFamily.Monospace)
                                Text("conf ${(s.confidence*100).toInt()}%  tort %.2f".format(s.tortuosity), color=Color.White.copy(0.6f), fontSize=10.sp, fontFamily=androidx.compose.ui.text.font.FontFamily.Monospace)
                                if(s.isUserAdjusted) Text("edited", color=Color(0xFF34C759), fontSize=9.sp, fontWeight=FontWeight.Bold)
                            }
                            Text("pts:%d  len:%.1fpx  start:(%.3f,%.3f)  end:(%.3f,%.3f)".format(s.points.size, s.arcLengthPx, start?.x?:0f, start?.y?:0f, end?.x?:0f, end?.y?:0f), color=Color.White.copy(0.72f), fontSize=10.sp, fontFamily=androidx.compose.ui.text.font.FontFamily.Monospace)
                        }
                    }
                }
            }
            // Bottom bar: small Retake left + large Use button right
            Row(horizontalArrangement=Arrangement.spacedBy(10.dp), verticalAlignment=Alignment.CenterVertically){
                Button(onClick=onRetake, modifier=Modifier.width(112.dp).height(48.dp), shape=RoundedCornerShape(24.dp), colors=ButtonDefaults.buttonColors(containerColor=Color.White.copy(alpha=0.10f)), border=androidx.compose.foundation.BorderStroke(1.dp, Color.White.copy(0.18f))){
                    Row(horizontalArrangement=Arrangement.spacedBy(6.dp), verticalAlignment=Alignment.CenterVertically){ Icon(Icons.Default.Refresh, null, tint=Color.White, modifier=Modifier.size(16.dp)); Text(LocalizedStrings.tr("result.retake", selectedLanguage), color=Color.White, fontSize=13.sp, fontWeight=FontWeight.SemiBold) }
                }
                Button(onClick={ onConfirmAdjustedLines(); showDebug=true }, modifier=Modifier.weight(1f).height(48.dp), shape=RoundedCornerShape(24.dp), colors=ButtonDefaults.buttonColors(containerColor=Color(0xFF34C759))){
                    Row(horizontalArrangement=Arrangement.spacedBy(8.dp), verticalAlignment=Alignment.CenterVertically){ Icon(Icons.Default.Check, null, tint=Color.Black, modifier=Modifier.size(16.dp)); Text(LocalizedStrings.tr("result.useAdjusted", selectedLanguage), color=Color.Black, fontWeight=FontWeight.Bold, fontSize=14.sp) }
                }
            }
        }
    }
}

private fun parseHex(hex: String): Color { val c=hex.removePrefix("#").toLongOrNull(16)?:0xFFFFFF; return Color(0xFF000000 or c) }

@Composable private fun ChipButton(label: String, icon: androidx.compose.ui.graphics.vector.ImageVector, emphasized: Boolean, onClick: ()->Unit){
    val bg=if(emphasized) Color(0xFF34C759) else Color.White.copy(0.12f); val fg=if(emphasized) Color.Black else Color.White
    Row(Modifier.clip(CircleShape).background(bg).border(1.dp, Color.White.copy(0.2f), CircleShape).clickable{onClick()}.padding(horizontal=10.dp, vertical=8.dp), verticalAlignment=Alignment.CenterVertically, horizontalArrangement=Arrangement.spacedBy(4.dp)){ Icon(icon,null,tint=fg,modifier=Modifier.size(16.dp)); Text(label,color=fg,fontSize=13.sp,fontWeight=FontWeight.SemiBold) }
}
@Composable private fun DragHintOverlay(modifier: Modifier=Modifier){
    Column(modifier, horizontalAlignment=Alignment.CenterHorizontally, verticalArrangement=Arrangement.spacedBy(6.dp)){
        Box(Modifier.width(88.dp).height(10.dp).clip(CircleShape).background(Color.White.copy(0.12f))) {}
        Text(LocalizedStrings.tr("result.dragHint"), color=Color.White, fontSize=12.sp, fontWeight=FontWeight.SemiBold, modifier=Modifier.clip(CircleShape).background(Color.Black.copy(0.55f)).padding(horizontal=10.dp, vertical=4.dp))
    }
}
