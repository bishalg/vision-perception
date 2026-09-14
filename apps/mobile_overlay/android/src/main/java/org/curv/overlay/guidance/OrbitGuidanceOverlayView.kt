package org.curv.overlay.guidance

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.curv.overlay.hud.OverlayEmerald

/**
 * Real-time HUD for Android Mode B 3D Hand Mesh & Orbit Scan.
 * Parity with iOS OrbitGuidanceOverlayView.swift:
 * - Overall progress % & progress bar
 * - 6 discrete angle pips
 * - Animated motion direction arrows
 * - Real-time good/bad angle feedback
 * - Educational coaching hints
 */
@Composable
fun OrbitGuidanceOverlayView(
    session: OrbitCaptureSession,
    state: OrbitGuidanceState,
    modifier: Modifier = Modifier
) {
    val infiniteTransition = rememberInfiniteTransition(label = "orbitPulse")
    val pulseScale by infiniteTransition.animateFloat(
        initialValue = 1.0f,
        targetValue = 1.08f,
        animationSpec = infiniteRepeatable(
            animation = tween(800, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "pulseScale"
    )

    val activeBin = when (state) {
        is OrbitGuidanceState.Coach -> state.bin
        is OrbitGuidanceState.HoldSteady -> state.bin
        is OrbitGuidanceState.Capturing -> state.bin
        else -> OrbitBin.all.firstOrNull { !session.filledBins.contains(it) }
    }

    val isGoodAngle = state is OrbitGuidanceState.HoldSteady || state is OrbitGuidanceState.Capturing
    val isRejected = state is OrbitGuidanceState.Rejected
    val progressPercent = (session.coverageFraction * 100).toInt()

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(top = 80.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // ── Top Header: Overall % and discrete angle pips ──
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(Color.Black.copy(alpha = 0.68f))
                .border(0.8.dp, Color.White.copy(alpha = 0.20f), RoundedCornerShape(16.dp))
                .padding(horizontal = 14.dp, vertical = 10.dp)
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "3D HAND MESH SCAN",
                        color = Color.White.copy(alpha = 0.85f),
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Black
                    )
                    Text(
                        text = "$progressPercent% • ${session.filledBins.size}/${OrbitBin.all.size}",
                        color = if (session.coverageFraction >= 1.0f) OverlayEmerald else Color.White,
                        fontSize = 12.sp,
                        fontWeight = FontWeight.Bold,
                        fontFamily = FontFamily.Monospace
                    )
                }

                // Progress track
                LinearProgressIndicator(
                    progress = { session.coverageFraction },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(4.dp)
                        .clip(CircleShape),
                    color = OverlayEmerald,
                    trackColor = Color.White.copy(alpha = 0.18f)
                )

                // 6 Angle Pips
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    OrbitBin.all.forEach { bin ->
                        val isDone = session.filledBins.contains(bin)
                        val isActive = (bin == activeBin && !isDone)
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .clip(RoundedCornerShape(12.dp))
                                .background(
                                    when {
                                        isDone -> Color(0xFF1E8E3E).copy(alpha = 0.85f)
                                        isActive -> Color.White.copy(alpha = 0.22f)
                                        else -> Color.White.copy(alpha = 0.08f)
                                    }
                                )
                                .border(
                                    if (isActive) 1.4.dp else 0.8.dp,
                                    when {
                                        isDone -> OverlayEmerald
                                        isActive -> if (isGoodAngle) OverlayEmerald else Color.White.copy(alpha = 0.6f)
                                        else -> Color.Transparent
                                    },
                                    RoundedCornerShape(12.dp)
                                )
                                .padding(vertical = 4.dp),
                            contentAlignment = Alignment.Center
                        ) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(2.dp)
                            ) {
                                if (isDone) {
                                    Icon(
                                        Icons.Filled.Check,
                                        contentDescription = null,
                                        tint = Color.White,
                                        modifier = Modifier.size(8.dp)
                                    )
                                } else {
                                    Text(
                                        text = bin.arrowSymbol,
                                        color = if (isActive) (if (isGoodAngle) OverlayEmerald else Color.White) else Color.White.copy(alpha = 0.5f),
                                        fontSize = 9.sp,
                                        fontWeight = FontWeight.Bold
                                    )
                                }
                                Text(
                                    text = bin.shortLabel,
                                    color = if (isDone || isActive) Color.White else Color.White.copy(alpha = 0.55f),
                                    fontSize = 8.sp,
                                    fontWeight = if (isActive) FontWeight.Bold else FontWeight.Medium
                                )
                            }
                        }
                    }
                }
            }
        }

        Spacer(modifier = Modifier.weight(1f))

        // ── Center Motion Direction & Coaching Feedback ──
        if (activeBin != null && session.coverageFraction < 1.0f) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.padding(horizontal = 24.dp)
            ) {
                // Animated Direction Indicator
                Box(
                    modifier = Modifier
                        .size(72.dp)
                        .scale(if (isGoodAngle) 1.0f else pulseScale)
                        .clip(CircleShape)
                        .background(Color.Black.copy(alpha = 0.72f))
                        .border(
                            if (isGoodAngle) 3.dp else 1.5.dp,
                            if (isGoodAngle) OverlayEmerald else (if (isRejected) Color.Red else Color.White.copy(alpha = 0.4f)),
                            CircleShape
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = activeBin.arrowSymbol,
                        fontSize = 34.sp,
                        fontWeight = FontWeight.Bold,
                        color = if (isGoodAngle) OverlayEmerald else Color.White
                    )
                }

                // Status Badge
                when (state) {
                    is OrbitGuidanceState.HoldSteady -> {
                        Box(
                            modifier = Modifier
                                .clip(CircleShape)
                                .background(Color(0xFF1E8E3E).copy(alpha = 0.75f))
                                .border(1.5.dp, OverlayEmerald, CircleShape)
                                .padding(horizontal = 14.dp, vertical = 7.dp)
                        ) {
                            Text(
                                text = "PERFECT ANGLE — HOLD STEADY (${(state.progress * 100).toInt()}%)",
                                color = Color.White,
                                fontSize = 12.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                    is OrbitGuidanceState.Rejected -> {
                        Box(
                            modifier = Modifier
                                .clip(CircleShape)
                                .background(Color.Red.copy(alpha = 0.7f))
                                .border(1.dp, Color.Red, CircleShape)
                                .padding(horizontal = 12.dp, vertical = 6.dp)
                        ) {
                            Text(
                                text = state.reason.uppercase(),
                                color = Color.White,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                    is OrbitGuidanceState.Capturing -> {
                        Box(
                            modifier = Modifier
                                .clip(CircleShape)
                                .background(Color.Black.copy(alpha = 0.75f))
                                .border(1.dp, OverlayEmerald, CircleShape)
                                .padding(horizontal = 12.dp, vertical = 6.dp)
                        ) {
                            Text(
                                text = "RECORDING 3D KEYFRAME...",
                                color = Color.White,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                    else -> {
                        Box(
                            modifier = Modifier
                                .clip(CircleShape)
                                .background(Color.Black.copy(alpha = 0.65f))
                                .border(1.dp, Color.White.copy(alpha = 0.35f), CircleShape)
                                .padding(horizontal = 14.dp, vertical = 7.dp)
                        ) {
                            Text(
                                text = activeBin.displayName.uppercase(),
                                color = Color.White,
                                fontSize = 12.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                }

                // Educational Coaching Tip
                Box(
                    modifier = Modifier
                        .clip(CircleShape)
                        .background(Color.Black.copy(alpha = 0.55f))
                        .border(0.6.dp, Color.White.copy(alpha = 0.16f), CircleShape)
                        .padding(horizontal = 14.dp, vertical = 6.dp)
                ) {
                    Text(
                        text = activeBin.coachingTip,
                        color = Color.White.copy(alpha = 0.85f),
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Medium,
                        textAlign = TextAlign.Center
                    )
                }
            }
        } else if (session.coverageFraction >= 1.0f) {
            Column(
                modifier = Modifier
                    .clip(RoundedCornerShape(18.dp))
                    .background(Color.Black.copy(alpha = 0.75f))
                    .border(1.5.dp, OverlayEmerald.copy(alpha = 0.6f), RoundedCornerShape(18.dp))
                    .padding(16.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                Icon(
                    Icons.Filled.CheckCircle,
                    contentDescription = null,
                    tint = OverlayEmerald,
                    modifier = Modifier.size(44.dp)
                )
                Text(
                    text = "3D SCAN COMPLETE",
                    color = Color.White,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.Bold
                )
                Text(
                    text = "Metric parallax relief map computed successfully",
                    color = Color.White.copy(alpha = 0.8f),
                    fontSize = 11.sp
                )
            }
        }

        Spacer(modifier = Modifier.weight(1f))
    }
}
