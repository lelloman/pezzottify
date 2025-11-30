package com.lelloman.pezzottify.android.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily

/**
 * Creates a Typography instance for the given font family.
 *
 * Font families available:
 * - System: Default system fonts (Roboto on most Android devices)
 * - SansSerif: Clean sans-serif font
 * - Serif: Classic serif font for a traditional look
 * - Monospace: Fixed-width font for a developer/technical feel
 */
fun createTypography(appFontFamily: AppFontFamily): Typography {
    val fontFamily = when (appFontFamily) {
        AppFontFamily.System -> FontFamily.Default
        AppFontFamily.SansSerif -> FontFamily.SansSerif
        AppFontFamily.Serif -> FontFamily.Serif
        AppFontFamily.Monospace -> FontFamily.Monospace
    }

    // Adjust weights for different font families to maintain visual consistency
    val displayWeight = when (appFontFamily) {
        AppFontFamily.Monospace -> FontWeight.Light  // Monospace looks better lighter
        AppFontFamily.Serif -> FontWeight.Normal
        else -> FontWeight.Normal
    }

    val headlineWeight = when (appFontFamily) {
        AppFontFamily.Monospace -> FontWeight.Medium
        AppFontFamily.Serif -> FontWeight.SemiBold
        else -> FontWeight.SemiBold
    }

    val titleWeight = when (appFontFamily) {
        AppFontFamily.Monospace -> FontWeight.Medium
        AppFontFamily.Serif -> FontWeight.Medium
        else -> FontWeight.SemiBold
    }

    return Typography(
        // Display styles - Large, prominent text
        displayLarge = TextStyle(
            fontFamily = fontFamily,
            fontWeight = displayWeight,
            fontSize = 57.sp,
            lineHeight = 64.sp,
            letterSpacing = (-0.25).sp
        ),
        displayMedium = TextStyle(
            fontFamily = fontFamily,
            fontWeight = displayWeight,
            fontSize = 45.sp,
            lineHeight = 52.sp,
            letterSpacing = 0.sp
        ),
        displaySmall = TextStyle(
            fontFamily = fontFamily,
            fontWeight = displayWeight,
            fontSize = 36.sp,
            lineHeight = 44.sp,
            letterSpacing = 0.sp
        ),

        // Headline styles - Section headers
        headlineLarge = TextStyle(
            fontFamily = fontFamily,
            fontWeight = headlineWeight,
            fontSize = 32.sp,
            lineHeight = 40.sp,
            letterSpacing = 0.sp
        ),
        headlineMedium = TextStyle(
            fontFamily = fontFamily,
            fontWeight = headlineWeight,
            fontSize = 28.sp,
            lineHeight = 36.sp,
            letterSpacing = 0.sp
        ),
        headlineSmall = TextStyle(
            fontFamily = fontFamily,
            fontWeight = headlineWeight,
            fontSize = 24.sp,
            lineHeight = 32.sp,
            letterSpacing = 0.sp
        ),

        // Title styles - Card titles, list items
        titleLarge = TextStyle(
            fontFamily = fontFamily,
            fontWeight = titleWeight,
            fontSize = 22.sp,
            lineHeight = 28.sp,
            letterSpacing = 0.sp
        ),
        titleMedium = TextStyle(
            fontFamily = fontFamily,
            fontWeight = FontWeight.Medium,
            fontSize = 16.sp,
            lineHeight = 24.sp,
            letterSpacing = 0.15.sp
        ),
        titleSmall = TextStyle(
            fontFamily = fontFamily,
            fontWeight = FontWeight.Medium,
            fontSize = 14.sp,
            lineHeight = 20.sp,
            letterSpacing = 0.1.sp
        ),

        // Body styles - Main content text
        bodyLarge = TextStyle(
            fontFamily = fontFamily,
            fontWeight = FontWeight.Normal,
            fontSize = 16.sp,
            lineHeight = 24.sp,
            letterSpacing = 0.5.sp
        ),
        bodyMedium = TextStyle(
            fontFamily = fontFamily,
            fontWeight = FontWeight.Normal,
            fontSize = 14.sp,
            lineHeight = 20.sp,
            letterSpacing = 0.25.sp
        ),
        bodySmall = TextStyle(
            fontFamily = fontFamily,
            fontWeight = FontWeight.Normal,
            fontSize = 12.sp,
            lineHeight = 16.sp,
            letterSpacing = 0.4.sp
        ),

        // Label styles - Buttons, tabs, labels
        labelLarge = TextStyle(
            fontFamily = fontFamily,
            fontWeight = FontWeight.Medium,
            fontSize = 14.sp,
            lineHeight = 20.sp,
            letterSpacing = 0.1.sp
        ),
        labelMedium = TextStyle(
            fontFamily = fontFamily,
            fontWeight = FontWeight.Medium,
            fontSize = 12.sp,
            lineHeight = 16.sp,
            letterSpacing = 0.5.sp
        ),
        labelSmall = TextStyle(
            fontFamily = fontFamily,
            fontWeight = FontWeight.Medium,
            fontSize = 11.sp,
            lineHeight = 16.sp,
            letterSpacing = 0.5.sp
        )
    )
}

/**
 * Default typography using system fonts.
 * Used when no specific font family is selected.
 */
val DefaultTypography = createTypography(AppFontFamily.System)
