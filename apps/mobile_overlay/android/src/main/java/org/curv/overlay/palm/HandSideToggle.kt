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
    onSelect: (HandSide) -> Unit,
    modifier: Modifier = Modifier
) {
    val isLeftActive = selection == HandSide.LEFT || (selection == HandSide.AUTO && autoDetected == HandSide.LEFT)
    val isRightActive = selection == HandSide.RIGHT || (selection == HandSide.AUTO && autoDetected == HandSide.RIGHT)

    Row(
        modifier = modifier
            .clip(Capsule)
            .background(Color.Black.copy(alpha = 0.42f))
            .border(0.8.dp, Color.White.copy(alpha = 0.18f), Capsule)
            .padding(2.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        SideButton(
            title = "L",
            isActive = isLeftActive,
            isLocked = selection == HandSide.LEFT,
            isAuto = selection == HandSide.AUTO,
            onClick = {
                if (selection == HandSide.LEFT) onSelect(HandSide.AUTO)
                else onSelect(HandSide.LEFT)
            }
        )
        SideButton(
            title = "R",
            isActive = isRightActive,
            isLocked = selection == HandSide.RIGHT,
            isAuto = selection == HandSide.AUTO,
            onClick = {
                if (selection == HandSide.RIGHT) onSelect(HandSide.AUTO)
                else onSelect(HandSide.RIGHT)
            }
        )
    }
}

@Composable
private fun SideButton(
    title: String,
    isActive: Boolean,
    isLocked: Boolean,
    isAuto: Boolean,
    onClick: () -> Unit
) {
    Box(
        modifier = Modifier
            .clip(Capsule)
            .background(
                when {
                    isLocked -> Color.White
                    isActive -> Color.White.copy(alpha = 0.16f)
                    else -> Color.Transparent
                }
            )
            .clickable { onClick() }
            .padding(horizontal = 8.dp, vertical = 4.dp),
        contentAlignment = Alignment.Center
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(3.dp)
        ) {
            Text(
                text = title,
                color = if (isLocked) Color.Black else if (isActive) Color.White else Color.White.copy(alpha = 0.45f),
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold
            )
            if (isActive && !isLocked && isAuto) {
                Box(
                    modifier = Modifier
                        .size(4.dp)
                        .clip(androidx.compose.foundation.shape.CircleShape)
                        .background(Color(0xFF34C759))
                )
            }
        }
    }
}
