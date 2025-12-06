package com.lelloman.pezzottify.android.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp

private val AppShapes = Shapes(
    extraSmall = RoundedCornerShape(4.dp),
    small = RoundedCornerShape(8.dp),
    medium = RoundedCornerShape(12.dp),
    large = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(24.dp)
)

@Composable
fun PezzottifyTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    themeMode: ThemeMode = ThemeMode.System,
    colorPalette: ColorPalette = ColorPalette.Classic,
    fontFamily: AppFontFamily = AppFontFamily.System,
    // Dynamic color is available on Android 12+
    // Set to false by default for consistent branding
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit
) {
    // Determine if we should use dark theme based on ThemeMode
    val useDarkTheme = when (themeMode) {
        ThemeMode.System -> darkTheme
        ThemeMode.Light -> false
        ThemeMode.Dark -> true
        ThemeMode.Amoled -> true  // AMOLED is always dark
    }

    val colorScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val context = LocalContext.current
            val scheme = if (useDarkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
            if (themeMode == ThemeMode.Amoled) scheme.withAmoledBlacks() else scheme
        }

        else -> {
            val scheme = getColorScheme(colorPalette, useDarkTheme)
            if (themeMode == ThemeMode.Amoled) scheme.withAmoledBlacks() else scheme
        }
    }

    val typography = createTypography(fontFamily)

    MaterialTheme(
        colorScheme = colorScheme,
        typography = typography,
        shapes = AppShapes,
        content = content
    )
}
