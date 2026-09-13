package org.curv.palmapp.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/**
 * Palm app Material3 theme.
 *
 * Brand accent is the signature palm green; everything else comes from the
 * Material3 tonal system so surfaces, sheets, and dialogs feel native and
 * adapt to light/dark mode automatically. All screens follow the system
 * theme on launch (`darkTheme` defaults to `isSystemInDarkTheme()`); only
 * media canvases (camera preview, photo review backdrop, editor canvas) stay
 * dark for tracing contrast, with fixed light-on-dark treatments on top.
 */
val PalmGreen = Color(0xFF34C759)
val PalmGreenDark = Color(0xFF1F9D4D)

private val PalmDarkColorScheme = darkColorScheme(
    primary = PalmGreen,
    onPrimary = Color.Black,
    primaryContainer = Color(0xFF0B3B1E),
    onPrimaryContainer = Color(0xFFB9F5CB),
    secondary = Color(0xFF9BE3AE),
    tertiary = Color(0xFF06B6D4)
)

private val PalmLightColorScheme = lightColorScheme(
    primary = PalmGreenDark,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFC9F2D4),
    onPrimaryContainer = Color(0xFF0B3B1E),
    secondary = Color(0xFF3D7A4E),
    tertiary = Color(0xFF0E7490)
)

@Composable
fun PalmAppTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    MaterialTheme(
        colorScheme = if (darkTheme) PalmDarkColorScheme else PalmLightColorScheme,
        // Default M3 typography + shapes: native type scale and corner radii.
        content = content
    )
}
