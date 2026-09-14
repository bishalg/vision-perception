package org.curv.overlay.palm

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.Capsule
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun HandSideToggle(
    selection: HandSide,
    autoDetected: HandSide,
    isHandDetected: Boolean = false,
    onSelect: (HandSide) -> Unit,
    modifier: Modifier = Modifier
) {
    val isAuto = selection == HandSide.AUTO

    Row(
        modifier = modifier
            .clip(RoundedCornerShape(20.dp))
            .background(Color.Black.copy(alpha = 0.45f))
            .border(0.8.dp, Color.White.copy(alpha = 0.18f), RoundedCornerShape(20.dp))
            .padding(3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(3.dp)
    ) {
        // AUTO Mode Badge & Button
        Box(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(
                    if (isAuto) {
                        if (isHandDetected) Color(0xFF34C759).copy(alpha = 0.20f)
                        else Color.White.copy(alpha = 0.12f)
                    } else Color.Transparent
                )
                .border(
                    1.dp,
                    if (isAuto) {
                        if (isHandDetected) Color(0xFF34C759).copy(alpha = 0.60f)
                        else Color.White.copy(alpha = 0.22f)
                    } else Color.Transparent,
                    RoundedCornerShape(16.dp)
                )
                .clickable { onSelect(HandSide.AUTO) }
                .padding(horizontal = 9.dp, vertical = 5.dp),
            contentAlignment = Alignment.Center
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                if (isAuto) {
                    Box(
                        modifier = Modifier
                            .size(5.dp)
                            .clip(CircleShape)
                            .background(if (isHandDetected) Color(0xFF34C759) else Color(0xFFFFD60A))
                    )
                }
                Text(
                    text = if (isAuto) {
                        if (isHandDetected) {
                            if (autoDetected == HandSide.LEFT) "AUTO • L" else "AUTO • R"
                        } else "AUTO"
                    } else "AUTO",
                    color = if (isAuto) Color.White else Color.White.copy(alpha = 0.45f),
                    fontSize = 11.sp,
                    fontWeight = if (isAuto) FontWeight.Bold else FontWeight.Medium
                )
            }
        }

        // Divider
        Box(
            modifier = Modifier
                .width(1.dp)
                .height(12.dp)
                .background(Color.White.copy(alpha = 0.18f))
        )

        // L Button
        val isLeftLocked = selection == HandSide.LEFT
        val isLeftAutoActive = isAuto && isHandDetected && autoDetected == HandSide.LEFT
        Box(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(
                    when {
                        isLeftLocked -> Color.White
                        isLeftAutoActive -> Color(0xFF34C759).copy(alpha = 0.18f)
                        else -> Color.Transparent
                    }
                )
                .clickable {
                    if (selection == HandSide.LEFT) onSelect(HandSide.AUTO)
                    else onSelect(HandSide.LEFT)
                }
                .padding(horizontal = 8.dp, vertical = 5.dp),
            contentAlignment = Alignment.Center
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(2.dp)
            ) {
                Text(
                    text = "L",
                    color = when {
                        isLeftLocked -> Color.Black
                        isLeftAutoActive -> Color(0xFF34C759)
                        else -> Color.White.copy(alpha = 0.45f)
                    },
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Bold
                )
                if (isLeftAutoActive) {
                    Box(
                        modifier = Modifier
                            .size(4.dp)
                            .clip(CircleShape)
                            .background(Color(0xFF34C759))
                    )
                }
            }
        }

        // R Button
        val isRightLocked = selection == HandSide.RIGHT
        val isRightAutoActive = isAuto && isHandDetected && autoDetected == HandSide.RIGHT
        Box(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(
                    when {
                        isRightLocked -> Color.White
                        isRightAutoActive -> Color(0xFF34C759).copy(alpha = 0.18f)
                        else -> Color.Transparent
                    }
                )
                .clickable {
                    if (selection == HandSide.RIGHT) onSelect(HandSide.AUTO)
                    else onSelect(HandSide.RIGHT)
                }
                .padding(horizontal = 8.dp, vertical = 5.dp),
            contentAlignment = Alignment.Center
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(2.dp)
            ) {
                Text(
                    text = "R",
                    color = when {
                        isRightLocked -> Color.Black
                        isRightAutoActive -> Color(0xFF34C759)
                        else -> Color.White.copy(alpha = 0.45f)
                    },
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Bold
                )
                if (isRightAutoActive) {
                    Box(
                        modifier = Modifier
                            .size(4.dp)
                            .clip(CircleShape)
                            .background(Color(0xFF34C759))
                    )
                }
            }
        }
    }
}

