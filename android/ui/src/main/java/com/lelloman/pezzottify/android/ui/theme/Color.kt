package com.lelloman.pezzottify.android.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color
import com.lelloman.pezzottify.android.domain.settings.ColorPalette

// ============================================================================
// CLASSIC PALETTE (Original Green Theme)
// ============================================================================

// Light Theme Colors - Classic
private val ClassicPrimary = Color(0xFF1DB954)       // Vibrant green - music/audio industry standard
private val ClassicOnPrimary = Color(0xFFFFFFFF)
private val ClassicPrimaryContainer = Color(0xFFB8F4D3)
private val ClassicOnPrimaryContainer = Color(0xFF002110)

private val ClassicSecondary = Color(0xFF4A6358)
private val ClassicOnSecondary = Color(0xFFFFFFFF)
private val ClassicSecondaryContainer = Color(0xFFCCE9D9)
private val ClassicOnSecondaryContainer = Color(0xFF072117)

private val ClassicTertiary = Color(0xFF3D6373)
private val ClassicOnTertiary = Color(0xFFFFFFFF)
private val ClassicTertiaryContainer = Color(0xFFC1E8FB)
private val ClassicOnTertiaryContainer = Color(0xFF001F29)

private val ClassicError = Color(0xFFBA1A1A)
private val ClassicOnError = Color(0xFFFFFFFF)
private val ClassicErrorContainer = Color(0xFFFFDAD6)
private val ClassicOnErrorContainer = Color(0xFF410002)

private val ClassicBackground = Color(0xFFFAFDFA)
private val ClassicOnBackground = Color(0xFF191C1A)
private val ClassicSurface = Color(0xFFFAFDFA)
private val ClassicOnSurface = Color(0xFF191C1A)
private val ClassicSurfaceVariant = Color(0xFFDCE5DD)
private val ClassicOnSurfaceVariant = Color(0xFF404943)

private val ClassicOutline = Color(0xFF707973)
private val ClassicOutlineVariant = Color(0xFFC0C9C1)

// Dark Theme Colors - Classic
private val ClassicPrimaryDark = Color(0xFF1DB954)
private val ClassicOnPrimaryDark = Color(0xFF003919)
private val ClassicPrimaryContainerDark = Color(0xFF005227)
private val ClassicOnPrimaryContainerDark = Color(0xFFB8F4D3)

private val ClassicSecondaryDark = Color(0xFFB0CDBE)
private val ClassicOnSecondaryDark = Color(0xFF1C352B)
private val ClassicSecondaryContainerDark = Color(0xFF334B41)
private val ClassicOnSecondaryContainerDark = Color(0xFFCCE9D9)

private val ClassicTertiaryDark = Color(0xFFA5CCDF)
private val ClassicOnTertiaryDark = Color(0xFF073543)
private val ClassicTertiaryContainerDark = Color(0xFF244C5A)
private val ClassicOnTertiaryContainerDark = Color(0xFFC1E8FB)

private val ClassicErrorDark = Color(0xFFFFB4AB)
private val ClassicOnErrorDark = Color(0xFF690005)
private val ClassicErrorContainerDark = Color(0xFF93000A)
private val ClassicOnErrorContainerDark = Color(0xFFFFDAD6)

private val ClassicBackgroundDark = Color(0xFF191C1A)
private val ClassicOnBackgroundDark = Color(0xFFE1E3DF)
private val ClassicSurfaceDark = Color(0xFF191C1A)
private val ClassicOnSurfaceDark = Color(0xFFE1E3DF)
private val ClassicSurfaceVariantDark = Color(0xFF404943)
private val ClassicOnSurfaceVariantDark = Color(0xFFC0C9C1)

private val ClassicOutlineDark = Color(0xFF8A938C)
private val ClassicOutlineVariantDark = Color(0xFF404943)

// ============================================================================
// OCEAN BLUE PALETTE
// ============================================================================

// Light Theme Colors - Ocean Blue
private val OceanPrimary = Color(0xFF0077B6)
private val OceanOnPrimary = Color(0xFFFFFFFF)
private val OceanPrimaryContainer = Color(0xFFCAE6FF)
private val OceanOnPrimaryContainer = Color(0xFF001E31)

private val OceanSecondary = Color(0xFF4E6070)
private val OceanOnSecondary = Color(0xFFFFFFFF)
private val OceanSecondaryContainer = Color(0xFFD1E4F8)
private val OceanOnSecondaryContainer = Color(0xFF091D2B)

private val OceanTertiary = Color(0xFF5A5F72)
private val OceanOnTertiary = Color(0xFFFFFFFF)
private val OceanTertiaryContainer = Color(0xFFDFE2F9)
private val OceanOnTertiaryContainer = Color(0xFF171B2C)

private val OceanBackground = Color(0xFFFCFCFF)
private val OceanOnBackground = Color(0xFF1A1C1E)
private val OceanSurface = Color(0xFFFCFCFF)
private val OceanOnSurface = Color(0xFF1A1C1E)
private val OceanSurfaceVariant = Color(0xFFDDE3EA)
private val OceanOnSurfaceVariant = Color(0xFF41484E)

private val OceanOutline = Color(0xFF72787E)
private val OceanOutlineVariant = Color(0xFFC1C7CE)

// Dark Theme Colors - Ocean Blue
private val OceanPrimaryDark = Color(0xFF90CDFF)
private val OceanOnPrimaryDark = Color(0xFF003450)
private val OceanPrimaryContainerDark = Color(0xFF004B71)
private val OceanOnPrimaryContainerDark = Color(0xFFCAE6FF)

private val OceanSecondaryDark = Color(0xFFB5C8DB)
private val OceanOnSecondaryDark = Color(0xFF1F3240)
private val OceanSecondaryContainerDark = Color(0xFF364858)
private val OceanOnSecondaryContainerDark = Color(0xFFD1E4F8)

private val OceanTertiaryDark = Color(0xFFC3C6DD)
private val OceanOnTertiaryDark = Color(0xFF2C3042)
private val OceanTertiaryContainerDark = Color(0xFF424659)
private val OceanOnTertiaryContainerDark = Color(0xFFDFE2F9)

private val OceanBackgroundDark = Color(0xFF1A1C1E)
private val OceanOnBackgroundDark = Color(0xFFE2E2E5)
private val OceanSurfaceDark = Color(0xFF1A1C1E)
private val OceanOnSurfaceDark = Color(0xFFE2E2E5)
private val OceanSurfaceVariantDark = Color(0xFF41484E)
private val OceanOnSurfaceVariantDark = Color(0xFFC1C7CE)

private val OceanOutlineDark = Color(0xFF8B9198)
private val OceanOutlineVariantDark = Color(0xFF41484E)

// ============================================================================
// SUNSET CORAL PALETTE
// ============================================================================

// Light Theme Colors - Sunset Coral
private val SunsetPrimary = Color(0xFFE85D4C)
private val SunsetOnPrimary = Color(0xFFFFFFFF)
private val SunsetPrimaryContainer = Color(0xFFFFDAD5)
private val SunsetOnPrimaryContainer = Color(0xFF410003)

private val SunsetSecondary = Color(0xFF775651)
private val SunsetOnSecondary = Color(0xFFFFFFFF)
private val SunsetSecondaryContainer = Color(0xFFFFDAD5)
private val SunsetOnSecondaryContainer = Color(0xFF2C1512)

private val SunsetTertiary = Color(0xFF6F5C2E)
private val SunsetOnTertiary = Color(0xFFFFFFFF)
private val SunsetTertiaryContainer = Color(0xFFFAE0A6)
private val SunsetOnTertiaryContainer = Color(0xFF251A00)

private val SunsetBackground = Color(0xFFFFFBFF)
private val SunsetOnBackground = Color(0xFF201A19)
private val SunsetSurface = Color(0xFFFFFBFF)
private val SunsetOnSurface = Color(0xFF201A19)
private val SunsetSurfaceVariant = Color(0xFFF5DDDA)
private val SunsetOnSurfaceVariant = Color(0xFF534341)

private val SunsetOutline = Color(0xFF857370)
private val SunsetOutlineVariant = Color(0xFFD8C2BF)

// Dark Theme Colors - Sunset Coral
private val SunsetPrimaryDark = Color(0xFFFFB4A9)
private val SunsetOnPrimaryDark = Color(0xFF690007)
private val SunsetPrimaryContainerDark = Color(0xFF930010)
private val SunsetOnPrimaryContainerDark = Color(0xFFFFDAD5)

private val SunsetSecondaryDark = Color(0xFFE7BDB7)
private val SunsetOnSecondaryDark = Color(0xFF442925)
private val SunsetSecondaryContainerDark = Color(0xFF5D3F3B)
private val SunsetOnSecondaryContainerDark = Color(0xFFFFDAD5)

private val SunsetTertiaryDark = Color(0xFFDCC48C)
private val SunsetOnTertiaryDark = Color(0xFF3D2E04)
private val SunsetTertiaryContainerDark = Color(0xFF554419)
private val SunsetOnTertiaryContainerDark = Color(0xFFFAE0A6)

private val SunsetBackgroundDark = Color(0xFF201A19)
private val SunsetOnBackgroundDark = Color(0xFFEDE0DE)
private val SunsetSurfaceDark = Color(0xFF201A19)
private val SunsetOnSurfaceDark = Color(0xFFEDE0DE)
private val SunsetSurfaceVariantDark = Color(0xFF534341)
private val SunsetOnSurfaceVariantDark = Color(0xFFD8C2BF)

private val SunsetOutlineDark = Color(0xFFA08C8A)
private val SunsetOutlineVariantDark = Color(0xFF534341)

// ============================================================================
// PURPLE HAZE PALETTE
// ============================================================================

// Light Theme Colors - Purple Haze
private val PurplePrimary = Color(0xFF7B4DFF)
private val PurpleOnPrimary = Color(0xFFFFFFFF)
private val PurplePrimaryContainer = Color(0xFFE8DDFF)
private val PurpleOnPrimaryContainer = Color(0xFF23005C)

private val PurpleSecondary = Color(0xFF635B70)
private val PurpleOnSecondary = Color(0xFFFFFFFF)
private val PurpleSecondaryContainer = Color(0xFFE9DEF8)
private val PurpleOnSecondaryContainer = Color(0xFF1F182B)

private val PurpleTertiary = Color(0xFF7E525E)
private val PurpleOnTertiary = Color(0xFFFFFFFF)
private val PurpleTertiaryContainer = Color(0xFFFFD9E1)
private val PurpleOnTertiaryContainer = Color(0xFF31101C)

private val PurpleBackground = Color(0xFFFFFBFF)
private val PurpleOnBackground = Color(0xFF1D1B1E)
private val PurpleSurface = Color(0xFFFFFBFF)
private val PurpleOnSurface = Color(0xFF1D1B1E)
private val PurpleSurfaceVariant = Color(0xFFE7E0EC)
private val PurpleOnSurfaceVariant = Color(0xFF49454E)

private val PurpleOutline = Color(0xFF7A757F)
private val PurpleOutlineVariant = Color(0xFFCBC4CF)

// Dark Theme Colors - Purple Haze
private val PurplePrimaryDark = Color(0xFFCFBDFF)
private val PurpleOnPrimaryDark = Color(0xFF3B0091)
private val PurplePrimaryContainerDark = Color(0xFF5528C5)
private val PurpleOnPrimaryContainerDark = Color(0xFFE8DDFF)

private val PurpleSecondaryDark = Color(0xFFCDC2DB)
private val PurpleOnSecondaryDark = Color(0xFF342D40)
private val PurpleSecondaryContainerDark = Color(0xFF4B4358)
private val PurpleOnSecondaryContainerDark = Color(0xFFE9DEF8)

private val PurpleTertiaryDark = Color(0xFFF0B8C5)
private val PurpleOnTertiaryDark = Color(0xFF4A2531)
private val PurpleTertiaryContainerDark = Color(0xFF643B47)
private val PurpleOnTertiaryContainerDark = Color(0xFFFFD9E1)

private val PurpleBackgroundDark = Color(0xFF1D1B1E)
private val PurpleOnBackgroundDark = Color(0xFFE6E1E6)
private val PurpleSurfaceDark = Color(0xFF1D1B1E)
private val PurpleOnSurfaceDark = Color(0xFFE6E1E6)
private val PurpleSurfaceVariantDark = Color(0xFF49454E)
private val PurpleOnSurfaceVariantDark = Color(0xFFCBC4CF)

private val PurpleOutlineDark = Color(0xFF948F99)
private val PurpleOutlineVariantDark = Color(0xFF49454E)

// ============================================================================
// AMOLED BLACK PALETTE (True black for OLED screens)
// ============================================================================

// Light Theme Colors - AMOLED (uses high contrast)
private val AmoledPrimary = Color(0xFF1DB954)
private val AmoledOnPrimary = Color(0xFFFFFFFF)
private val AmoledPrimaryContainer = Color(0xFFB8F4D3)
private val AmoledOnPrimaryContainer = Color(0xFF002110)

private val AmoledSecondary = Color(0xFF4A6358)
private val AmoledOnSecondary = Color(0xFFFFFFFF)
private val AmoledSecondaryContainer = Color(0xFFCCE9D9)
private val AmoledOnSecondaryContainer = Color(0xFF072117)

private val AmoledTertiary = Color(0xFF3D6373)
private val AmoledOnTertiary = Color(0xFFFFFFFF)
private val AmoledTertiaryContainer = Color(0xFFC1E8FB)
private val AmoledOnTertiaryContainer = Color(0xFF001F29)

private val AmoledBackground = Color(0xFFFAFDFA)
private val AmoledOnBackground = Color(0xFF191C1A)
private val AmoledSurface = Color(0xFFFAFDFA)
private val AmoledOnSurface = Color(0xFF191C1A)
private val AmoledSurfaceVariant = Color(0xFFDCE5DD)
private val AmoledOnSurfaceVariant = Color(0xFF404943)

private val AmoledOutline = Color(0xFF707973)
private val AmoledOutlineVariant = Color(0xFFC0C9C1)

// Dark Theme Colors - AMOLED (true black background)
private val AmoledPrimaryDark = Color(0xFF1DB954)
private val AmoledOnPrimaryDark = Color(0xFF003919)
private val AmoledPrimaryContainerDark = Color(0xFF005227)
private val AmoledOnPrimaryContainerDark = Color(0xFFB8F4D3)

private val AmoledSecondaryDark = Color(0xFFB0CDBE)
private val AmoledOnSecondaryDark = Color(0xFF1C352B)
private val AmoledSecondaryContainerDark = Color(0xFF334B41)
private val AmoledOnSecondaryContainerDark = Color(0xFFCCE9D9)

private val AmoledTertiaryDark = Color(0xFFA5CCDF)
private val AmoledOnTertiaryDark = Color(0xFF073543)
private val AmoledTertiaryContainerDark = Color(0xFF244C5A)
private val AmoledOnTertiaryContainerDark = Color(0xFFC1E8FB)

private val AmoledErrorDark = Color(0xFFFFB4AB)
private val AmoledOnErrorDark = Color(0xFF690005)
private val AmoledErrorContainerDark = Color(0xFF93000A)
private val AmoledOnErrorContainerDark = Color(0xFFFFDAD6)

private val AmoledBackgroundDark = Color(0xFF000000)  // True black
private val AmoledOnBackgroundDark = Color(0xFFE1E3DF)
private val AmoledSurfaceDark = Color(0xFF000000)      // True black
private val AmoledOnSurfaceDark = Color(0xFFE1E3DF)
private val AmoledSurfaceVariantDark = Color(0xFF1A1C1A)
private val AmoledOnSurfaceVariantDark = Color(0xFFC0C9C1)

private val AmoledOutlineDark = Color(0xFF8A938C)
private val AmoledOutlineVariantDark = Color(0xFF2A2D2A)

// ============================================================================
// COMMON ERROR COLORS (reused across palettes)
// ============================================================================

private val CommonError = Color(0xFFBA1A1A)
private val CommonOnError = Color(0xFFFFFFFF)
private val CommonErrorContainer = Color(0xFFFFDAD6)
private val CommonOnErrorContainer = Color(0xFF410002)

private val CommonErrorDark = Color(0xFFFFB4AB)
private val CommonOnErrorDark = Color(0xFF690005)
private val CommonErrorContainerDark = Color(0xFF93000A)
private val CommonOnErrorContainerDark = Color(0xFFFFDAD6)

// ============================================================================
// COLOR SCHEME FACTORIES
// ============================================================================

private val ClassicLightColorScheme = lightColorScheme(
    primary = ClassicPrimary,
    onPrimary = ClassicOnPrimary,
    primaryContainer = ClassicPrimaryContainer,
    onPrimaryContainer = ClassicOnPrimaryContainer,
    secondary = ClassicSecondary,
    onSecondary = ClassicOnSecondary,
    secondaryContainer = ClassicSecondaryContainer,
    onSecondaryContainer = ClassicOnSecondaryContainer,
    tertiary = ClassicTertiary,
    onTertiary = ClassicOnTertiary,
    tertiaryContainer = ClassicTertiaryContainer,
    onTertiaryContainer = ClassicOnTertiaryContainer,
    error = CommonError,
    onError = CommonOnError,
    errorContainer = CommonErrorContainer,
    onErrorContainer = CommonOnErrorContainer,
    background = ClassicBackground,
    onBackground = ClassicOnBackground,
    surface = ClassicSurface,
    onSurface = ClassicOnSurface,
    surfaceVariant = ClassicSurfaceVariant,
    onSurfaceVariant = ClassicOnSurfaceVariant,
    outline = ClassicOutline,
    outlineVariant = ClassicOutlineVariant,
)

private val ClassicDarkColorScheme = darkColorScheme(
    primary = ClassicPrimaryDark,
    onPrimary = ClassicOnPrimaryDark,
    primaryContainer = ClassicPrimaryContainerDark,
    onPrimaryContainer = ClassicOnPrimaryContainerDark,
    secondary = ClassicSecondaryDark,
    onSecondary = ClassicOnSecondaryDark,
    secondaryContainer = ClassicSecondaryContainerDark,
    onSecondaryContainer = ClassicOnSecondaryContainerDark,
    tertiary = ClassicTertiaryDark,
    onTertiary = ClassicOnTertiaryDark,
    tertiaryContainer = ClassicTertiaryContainerDark,
    onTertiaryContainer = ClassicOnTertiaryContainerDark,
    error = CommonErrorDark,
    onError = CommonOnErrorDark,
    errorContainer = CommonErrorContainerDark,
    onErrorContainer = CommonOnErrorContainerDark,
    background = ClassicBackgroundDark,
    onBackground = ClassicOnBackgroundDark,
    surface = ClassicSurfaceDark,
    onSurface = ClassicOnSurfaceDark,
    surfaceVariant = ClassicSurfaceVariantDark,
    onSurfaceVariant = ClassicOnSurfaceVariantDark,
    outline = ClassicOutlineDark,
    outlineVariant = ClassicOutlineVariantDark,
)

private val OceanLightColorScheme = lightColorScheme(
    primary = OceanPrimary,
    onPrimary = OceanOnPrimary,
    primaryContainer = OceanPrimaryContainer,
    onPrimaryContainer = OceanOnPrimaryContainer,
    secondary = OceanSecondary,
    onSecondary = OceanOnSecondary,
    secondaryContainer = OceanSecondaryContainer,
    onSecondaryContainer = OceanOnSecondaryContainer,
    tertiary = OceanTertiary,
    onTertiary = OceanOnTertiary,
    tertiaryContainer = OceanTertiaryContainer,
    onTertiaryContainer = OceanOnTertiaryContainer,
    error = CommonError,
    onError = CommonOnError,
    errorContainer = CommonErrorContainer,
    onErrorContainer = CommonOnErrorContainer,
    background = OceanBackground,
    onBackground = OceanOnBackground,
    surface = OceanSurface,
    onSurface = OceanOnSurface,
    surfaceVariant = OceanSurfaceVariant,
    onSurfaceVariant = OceanOnSurfaceVariant,
    outline = OceanOutline,
    outlineVariant = OceanOutlineVariant,
)

private val OceanDarkColorScheme = darkColorScheme(
    primary = OceanPrimaryDark,
    onPrimary = OceanOnPrimaryDark,
    primaryContainer = OceanPrimaryContainerDark,
    onPrimaryContainer = OceanOnPrimaryContainerDark,
    secondary = OceanSecondaryDark,
    onSecondary = OceanOnSecondaryDark,
    secondaryContainer = OceanSecondaryContainerDark,
    onSecondaryContainer = OceanOnSecondaryContainerDark,
    tertiary = OceanTertiaryDark,
    onTertiary = OceanOnTertiaryDark,
    tertiaryContainer = OceanTertiaryContainerDark,
    onTertiaryContainer = OceanOnTertiaryContainerDark,
    error = CommonErrorDark,
    onError = CommonOnErrorDark,
    errorContainer = CommonErrorContainerDark,
    onErrorContainer = CommonOnErrorContainerDark,
    background = OceanBackgroundDark,
    onBackground = OceanOnBackgroundDark,
    surface = OceanSurfaceDark,
    onSurface = OceanOnSurfaceDark,
    surfaceVariant = OceanSurfaceVariantDark,
    onSurfaceVariant = OceanOnSurfaceVariantDark,
    outline = OceanOutlineDark,
    outlineVariant = OceanOutlineVariantDark,
)

private val SunsetLightColorScheme = lightColorScheme(
    primary = SunsetPrimary,
    onPrimary = SunsetOnPrimary,
    primaryContainer = SunsetPrimaryContainer,
    onPrimaryContainer = SunsetOnPrimaryContainer,
    secondary = SunsetSecondary,
    onSecondary = SunsetOnSecondary,
    secondaryContainer = SunsetSecondaryContainer,
    onSecondaryContainer = SunsetOnSecondaryContainer,
    tertiary = SunsetTertiary,
    onTertiary = SunsetOnTertiary,
    tertiaryContainer = SunsetTertiaryContainer,
    onTertiaryContainer = SunsetOnTertiaryContainer,
    error = CommonError,
    onError = CommonOnError,
    errorContainer = CommonErrorContainer,
    onErrorContainer = CommonOnErrorContainer,
    background = SunsetBackground,
    onBackground = SunsetOnBackground,
    surface = SunsetSurface,
    onSurface = SunsetOnSurface,
    surfaceVariant = SunsetSurfaceVariant,
    onSurfaceVariant = SunsetOnSurfaceVariant,
    outline = SunsetOutline,
    outlineVariant = SunsetOutlineVariant,
)

private val SunsetDarkColorScheme = darkColorScheme(
    primary = SunsetPrimaryDark,
    onPrimary = SunsetOnPrimaryDark,
    primaryContainer = SunsetPrimaryContainerDark,
    onPrimaryContainer = SunsetOnPrimaryContainerDark,
    secondary = SunsetSecondaryDark,
    onSecondary = SunsetOnSecondaryDark,
    secondaryContainer = SunsetSecondaryContainerDark,
    onSecondaryContainer = SunsetOnSecondaryContainerDark,
    tertiary = SunsetTertiaryDark,
    onTertiary = SunsetOnTertiaryDark,
    tertiaryContainer = SunsetTertiaryContainerDark,
    onTertiaryContainer = SunsetOnTertiaryContainerDark,
    error = CommonErrorDark,
    onError = CommonOnErrorDark,
    errorContainer = CommonErrorContainerDark,
    onErrorContainer = CommonOnErrorContainerDark,
    background = SunsetBackgroundDark,
    onBackground = SunsetOnBackgroundDark,
    surface = SunsetSurfaceDark,
    onSurface = SunsetOnSurfaceDark,
    surfaceVariant = SunsetSurfaceVariantDark,
    onSurfaceVariant = SunsetOnSurfaceVariantDark,
    outline = SunsetOutlineDark,
    outlineVariant = SunsetOutlineVariantDark,
)

private val PurpleLightColorScheme = lightColorScheme(
    primary = PurplePrimary,
    onPrimary = PurpleOnPrimary,
    primaryContainer = PurplePrimaryContainer,
    onPrimaryContainer = PurpleOnPrimaryContainer,
    secondary = PurpleSecondary,
    onSecondary = PurpleOnSecondary,
    secondaryContainer = PurpleSecondaryContainer,
    onSecondaryContainer = PurpleOnSecondaryContainer,
    tertiary = PurpleTertiary,
    onTertiary = PurpleOnTertiary,
    tertiaryContainer = PurpleTertiaryContainer,
    onTertiaryContainer = PurpleOnTertiaryContainer,
    error = CommonError,
    onError = CommonOnError,
    errorContainer = CommonErrorContainer,
    onErrorContainer = CommonOnErrorContainer,
    background = PurpleBackground,
    onBackground = PurpleOnBackground,
    surface = PurpleSurface,
    onSurface = PurpleOnSurface,
    surfaceVariant = PurpleSurfaceVariant,
    onSurfaceVariant = PurpleOnSurfaceVariant,
    outline = PurpleOutline,
    outlineVariant = PurpleOutlineVariant,
)

private val PurpleDarkColorScheme = darkColorScheme(
    primary = PurplePrimaryDark,
    onPrimary = PurpleOnPrimaryDark,
    primaryContainer = PurplePrimaryContainerDark,
    onPrimaryContainer = PurpleOnPrimaryContainerDark,
    secondary = PurpleSecondaryDark,
    onSecondary = PurpleOnSecondaryDark,
    secondaryContainer = PurpleSecondaryContainerDark,
    onSecondaryContainer = PurpleOnSecondaryContainerDark,
    tertiary = PurpleTertiaryDark,
    onTertiary = PurpleOnTertiaryDark,
    tertiaryContainer = PurpleTertiaryContainerDark,
    onTertiaryContainer = PurpleOnTertiaryContainerDark,
    error = CommonErrorDark,
    onError = CommonOnErrorDark,
    errorContainer = CommonErrorContainerDark,
    onErrorContainer = CommonOnErrorContainerDark,
    background = PurpleBackgroundDark,
    onBackground = PurpleOnBackgroundDark,
    surface = PurpleSurfaceDark,
    onSurface = PurpleOnSurfaceDark,
    surfaceVariant = PurpleSurfaceVariantDark,
    onSurfaceVariant = PurpleOnSurfaceVariantDark,
    outline = PurpleOutlineDark,
    outlineVariant = PurpleOutlineVariantDark,
)

private val AmoledLightColorScheme = lightColorScheme(
    primary = AmoledPrimary,
    onPrimary = AmoledOnPrimary,
    primaryContainer = AmoledPrimaryContainer,
    onPrimaryContainer = AmoledOnPrimaryContainer,
    secondary = AmoledSecondary,
    onSecondary = AmoledOnSecondary,
    secondaryContainer = AmoledSecondaryContainer,
    onSecondaryContainer = AmoledOnSecondaryContainer,
    tertiary = AmoledTertiary,
    onTertiary = AmoledOnTertiary,
    tertiaryContainer = AmoledTertiaryContainer,
    onTertiaryContainer = AmoledOnTertiaryContainer,
    error = CommonError,
    onError = CommonOnError,
    errorContainer = CommonErrorContainer,
    onErrorContainer = CommonOnErrorContainer,
    background = AmoledBackground,
    onBackground = AmoledOnBackground,
    surface = AmoledSurface,
    onSurface = AmoledOnSurface,
    surfaceVariant = AmoledSurfaceVariant,
    onSurfaceVariant = AmoledOnSurfaceVariant,
    outline = AmoledOutline,
    outlineVariant = AmoledOutlineVariant,
)

private val AmoledDarkColorScheme = darkColorScheme(
    primary = AmoledPrimaryDark,
    onPrimary = AmoledOnPrimaryDark,
    primaryContainer = AmoledPrimaryContainerDark,
    onPrimaryContainer = AmoledOnPrimaryContainerDark,
    secondary = AmoledSecondaryDark,
    onSecondary = AmoledOnSecondaryDark,
    secondaryContainer = AmoledSecondaryContainerDark,
    onSecondaryContainer = AmoledOnSecondaryContainerDark,
    tertiary = AmoledTertiaryDark,
    onTertiary = AmoledOnTertiaryDark,
    tertiaryContainer = AmoledTertiaryContainerDark,
    onTertiaryContainer = AmoledOnTertiaryContainerDark,
    error = AmoledErrorDark,
    onError = AmoledOnErrorDark,
    errorContainer = AmoledErrorContainerDark,
    onErrorContainer = AmoledOnErrorContainerDark,
    background = AmoledBackgroundDark,
    onBackground = AmoledOnBackgroundDark,
    surface = AmoledSurfaceDark,
    onSurface = AmoledOnSurfaceDark,
    surfaceVariant = AmoledSurfaceVariantDark,
    onSurfaceVariant = AmoledOnSurfaceVariantDark,
    outline = AmoledOutlineDark,
    outlineVariant = AmoledOutlineVariantDark,
)

/**
 * Returns the appropriate ColorScheme for the given palette and dark mode setting.
 */
fun getColorScheme(palette: ColorPalette, darkTheme: Boolean): ColorScheme {
    return when (palette) {
        ColorPalette.Classic -> if (darkTheme) ClassicDarkColorScheme else ClassicLightColorScheme
        ColorPalette.OceanBlue -> if (darkTheme) OceanDarkColorScheme else OceanLightColorScheme
        ColorPalette.SunsetCoral -> if (darkTheme) SunsetDarkColorScheme else SunsetLightColorScheme
        ColorPalette.PurpleHaze -> if (darkTheme) PurpleDarkColorScheme else PurpleLightColorScheme
        ColorPalette.AmoledBlack -> if (darkTheme) AmoledDarkColorScheme else AmoledLightColorScheme
    }
}

/**
 * Returns preview colors for a palette to show in the settings UI.
 * Returns a list of 4 colors: primary, secondary, tertiary, and background.
 */
fun getPalettePreviewColors(palette: ColorPalette): List<Color> {
    return when (palette) {
        ColorPalette.Classic -> listOf(
            ClassicPrimary,
            ClassicSecondary,
            ClassicTertiary,
            ClassicBackground
        )
        ColorPalette.OceanBlue -> listOf(
            OceanPrimary,
            OceanSecondary,
            OceanTertiary,
            OceanBackground
        )
        ColorPalette.SunsetCoral -> listOf(
            SunsetPrimary,
            SunsetSecondary,
            SunsetTertiary,
            SunsetBackground
        )
        ColorPalette.PurpleHaze -> listOf(
            PurplePrimary,
            PurpleSecondary,
            PurpleTertiary,
            PurpleBackground
        )
        ColorPalette.AmoledBlack -> listOf(
            AmoledPrimary,
            AmoledSecondaryDark,
            AmoledBackgroundDark,  // True black
            AmoledSurfaceVariantDark
        )
    }
}
