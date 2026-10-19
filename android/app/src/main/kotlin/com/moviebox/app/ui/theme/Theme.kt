package com.moviebox.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

private val MovieBoxColors = darkColorScheme(
    primary = Color(0xFFCBA6F7),
    onPrimary = Color(0xFF1E1E2E),
    primaryContainer = Color(0xFF4C3F6E),
    onPrimaryContainer = Color(0xFFEADDFF),
    secondary = Color(0xFF89B4FA),
    onSecondary = Color(0xFF11131B),
    tertiary = Color(0xFF94E2D5),
    background = Color(0xFF11111B),
    onBackground = Color(0xFFE6E6F0),
    surface = Color(0xFF181825),
    onSurface = Color(0xFFE6E6F0),
    surfaceVariant = Color(0xFF313244),
    onSurfaceVariant = Color(0xFFC6C6D5),
    error = Color(0xFFF38BA8),
    onError = Color(0xFF11111B),
)

private val MovieBoxTypography = Typography(
    titleLarge = TextStyle(fontSize = 22.sp, fontWeight = FontWeight.SemiBold),
    titleMedium = TextStyle(fontSize = 16.sp, fontWeight = FontWeight.Medium),
    bodyMedium = TextStyle(fontSize = 14.sp),
    bodySmall = TextStyle(fontSize = 12.sp),
    labelSmall = TextStyle(fontSize = 11.sp, fontWeight = FontWeight.Medium),
)

@Composable
fun MovieBoxTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = MovieBoxColors,
        typography = MovieBoxTypography,
        content = content,
    )
}
