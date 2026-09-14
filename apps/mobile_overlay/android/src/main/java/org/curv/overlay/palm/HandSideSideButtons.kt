package org.curv.overlay.palm

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Compact bottom-left [ • L ] button beside telemetry chips.
 */
@Composable
fun LeftHandSideButton(
    selection: HandSide,
    autoDetected: HandSide,
    onSelect: (HandSide) -> Unit,
    modifier: Modifier = Modifier
) {
    val isActive = selection == HandSide.LEFT || (selection == HandSide.AUTO && autoDetected == HandSide.LEFT)
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(16.dp))
            .background(Color.Black.copy(alpha = 0.58f))
            .border(
                if (isActive) 1.4.dp else 0.8.dp,
                if (isActive) Color(0xFF34C759).copy(alpha = 0.8f) else Color.White.copy(alpha = 0.20f),
                RoundedCornerShape(16.dp)
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
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(5.dp)
                    .clip(CircleShape)
                    .background(if (isActive) Color(0xFF34C759) else Color.White.copy(alpha = 0.32f))
            )
            Text(
                text = "L",
                color = if (isActive) Color.White else Color.White.copy(alpha = 0.55f),
                fontSize = 11.sp,
                fontWeight = if (isActive) FontWeight.ExtraBold else FontWeight.Bold
            )
        }
    }
}

/**
 * Compact bottom-right [ R • ] button beside telemetry chips.
 */
@Composable
fun RightHandSideButton(
    selection: HandSide,
    autoDetected: HandSide,
    onSelect: (HandSide) -> Unit,
    modifier: Modifier = Modifier
) {
    val isActive = selection == HandSide.RIGHT || (selection == HandSide.AUTO && autoDetected == HandSide.RIGHT)
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(16.dp))
            .background(Color.Black.copy(alpha = 0.58f))
            .border(
                if (isActive) 1.4.dp else 0.8.dp,
                if (isActive) Color(0xFF34C759).copy(alpha = 0.8f) else Color.White.copy(alpha = 0.20f),
                RoundedCornerShape(16.dp)
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
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Text(
                text = "R",
                color = if (isActive) Color.White else Color.White.copy(alpha = 0.55f),
                fontSize = 11.sp,
                fontWeight = if (isActive) FontWeight.ExtraBold else FontWeight.Bold
            )
            Box(
                modifier = Modifier
                    .size(5.dp)
                    .clip(CircleShape)
                    .background(if (isActive) Color(0xFF34C759) else Color.White.copy(alpha = 0.32f))
            )
        }
    }
}
