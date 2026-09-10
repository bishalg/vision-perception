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
    Row(
        modifier = modifier
            .clip(Capsule)
            .background(Color.White.copy(alpha = 0.08f))
            .border(1.dp, Color.White.copy(alpha = 0.18f), Capsule)
            .padding(3.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        HandSide.values().forEach { side ->
            val isSel = side == selection
            val label = when (side) {
                HandSide.AUTO -> if (autoDetected != HandSide.AUTO) "Auto · ${autoDetected.name}" else "Auto"
                else -> side.name.lowercase().replaceFirstChar { it.uppercase() }
            }
            Box(
                modifier = Modifier
                    .clip(Capsule)
                    .background(if (isSel) Color.White else Color.Transparent)
                    .border(if (isSel) 0.dp else 1.dp, if (isSel) Color.Transparent else Color.White.copy(0.22f), Capsule)
                    .clickable { onSelect(side) }
                    .padding(horizontal = 10.dp, vertical = 7.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(label, color = if (isSel) Color.Black else Color.White.copy(0.85f), fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            }
        }
    }
}
