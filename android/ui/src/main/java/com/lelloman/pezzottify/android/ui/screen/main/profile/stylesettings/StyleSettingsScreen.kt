package com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import com.lelloman.pezzottify.android.ui.theme.getPalettePreviewColors
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

@Composable
fun StyleSettingsScreen(navController: NavController) {
    val viewModel = hiltViewModel<StyleSettingsViewModel>()
    StyleSettingsScreenInternal(
        state = viewModel.state,
        navController = navController,
        actions = viewModel,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun StyleSettingsScreenInternal(
    state: StateFlow<StyleSettingsState>,
    actions: StyleSettingsActions,
    navController: NavController,
) {
    val currentState by state.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Appearance") },
                navigationIcon = {
                    IconButton(onClick = { navController.popBackStack() }) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back"
                        )
                    }
                }
            )
        }
    ) { innerPadding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(horizontal = 16.dp)
        ) {
            // Color Palette Section
            item {
                Text(
                    text = "Color Palette",
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(vertical = 16.dp)
                )
            }

            items(ColorPalette.entries) { palette ->
                PaletteItem(
                    palette = palette,
                    isSelected = currentState.colorPalette == palette,
                    onClick = { actions.selectColorPalette(palette) }
                )
            }

            item {
                HorizontalDivider(modifier = Modifier.padding(vertical = 24.dp))
            }

            // Font Family Section
            item {
                Text(
                    text = "Font",
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(bottom = 16.dp)
                )
            }

            items(AppFontFamily.entries) { fontFamily ->
                FontFamilyItem(
                    fontFamily = fontFamily,
                    isSelected = currentState.fontFamily == fontFamily,
                    onClick = { actions.selectFontFamily(fontFamily) }
                )
            }

            item {
                Spacer(modifier = Modifier.height(32.dp))
            }
        }
    }
}

@Composable
private fun PaletteItem(
    palette: ColorPalette,
    isSelected: Boolean,
    onClick: () -> Unit,
) {
    val colors = getPalettePreviewColors(palette)
    val borderColor = if (isSelected) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.outlineVariant
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .border(
                width = if (isSelected) 2.dp else 1.dp,
                color = borderColor,
                shape = RoundedCornerShape(12.dp)
            )
            .clickable(onClick = onClick)
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Color preview squares
        Row(
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            colors.forEach { color ->
                Box(
                    modifier = Modifier
                        .size(32.dp)
                        .clip(RoundedCornerShape(6.dp))
                        .background(color)
                        .border(
                            width = 1.dp,
                            color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f),
                            shape = RoundedCornerShape(6.dp)
                        )
                )
            }
        }

        Spacer(modifier = Modifier.width(16.dp))

        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = getPaletteName(palette),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                text = getPaletteDescription(palette),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }

        if (isSelected) {
            Icon(
                imageVector = Icons.Default.Check,
                contentDescription = "Selected",
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(24.dp)
            )
        }
    }

    Spacer(modifier = Modifier.height(8.dp))
}

@Composable
private fun FontFamilyItem(
    fontFamily: AppFontFamily,
    isSelected: Boolean,
    onClick: () -> Unit,
) {
    val borderColor = if (isSelected) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.outlineVariant
    }

    val previewFontFamily = when (fontFamily) {
        AppFontFamily.System -> FontFamily.Default
        AppFontFamily.SansSerif -> FontFamily.SansSerif
        AppFontFamily.Serif -> FontFamily.Serif
        AppFontFamily.Monospace -> FontFamily.Monospace
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .border(
                width = if (isSelected) 2.dp else 1.dp,
                color = borderColor,
                shape = RoundedCornerShape(12.dp)
            )
            .clickable(onClick = onClick)
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Font preview letter
        Box(
            modifier = Modifier
                .size(48.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.primaryContainer),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = "Aa",
                style = MaterialTheme.typography.titleLarge.copy(
                    fontFamily = previewFontFamily
                ),
                color = MaterialTheme.colorScheme.onPrimaryContainer
            )
        }

        Spacer(modifier = Modifier.width(16.dp))

        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = getFontFamilyName(fontFamily),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                text = getFontFamilyDescription(fontFamily),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }

        if (isSelected) {
            Icon(
                imageVector = Icons.Default.Check,
                contentDescription = "Selected",
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(24.dp)
            )
        }
    }

    Spacer(modifier = Modifier.height(8.dp))
}

private fun getPaletteName(palette: ColorPalette): String = when (palette) {
    ColorPalette.Classic -> "Classic Green"
    ColorPalette.OceanBlue -> "Ocean Blue"
    ColorPalette.SunsetCoral -> "Sunset Coral"
    ColorPalette.PurpleHaze -> "Purple Haze"
    ColorPalette.AmoledBlack -> "AMOLED Black"
}

private fun getPaletteDescription(palette: ColorPalette): String = when (palette) {
    ColorPalette.Classic -> "Vibrant green, music industry standard"
    ColorPalette.OceanBlue -> "Cool, calming blue tones"
    ColorPalette.SunsetCoral -> "Warm coral and orange"
    ColorPalette.PurpleHaze -> "Rich violet and purple"
    ColorPalette.AmoledBlack -> "True black for OLED screens"
}

private fun getFontFamilyName(fontFamily: AppFontFamily): String = when (fontFamily) {
    AppFontFamily.System -> "System Default"
    AppFontFamily.SansSerif -> "Sans Serif"
    AppFontFamily.Serif -> "Serif"
    AppFontFamily.Monospace -> "Monospace"
}

private fun getFontFamilyDescription(fontFamily: AppFontFamily): String = when (fontFamily) {
    AppFontFamily.System -> "Default device font"
    AppFontFamily.SansSerif -> "Clean, modern sans-serif"
    AppFontFamily.Serif -> "Classic, traditional style"
    AppFontFamily.Monospace -> "Fixed-width, developer style"
}

@Composable
@Preview(showBackground = true)
private fun StyleSettingsScreenPreview() {
    PezzottifyTheme {
        StyleSettingsScreenInternal(
            state = MutableStateFlow(
                StyleSettingsState(
                    colorPalette = ColorPalette.Classic,
                    fontFamily = AppFontFamily.System,
                )
            ),
            navController = rememberNavController(),
            actions = object : StyleSettingsActions {
                override fun selectColorPalette(colorPalette: ColorPalette) {}
                override fun selectFontFamily(fontFamily: AppFontFamily) {}
            },
        )
    }
}

@Composable
@Preview(showBackground = true)
private fun StyleSettingsScreenPreviewDark() {
    PezzottifyTheme(darkTheme = true, colorPalette = ColorPalette.PurpleHaze) {
        StyleSettingsScreenInternal(
            state = MutableStateFlow(
                StyleSettingsState(
                    colorPalette = ColorPalette.PurpleHaze,
                    fontFamily = AppFontFamily.Monospace,
                )
            ),
            navController = rememberNavController(),
            actions = object : StyleSettingsActions {
                override fun selectColorPalette(colorPalette: ColorPalette) {}
                override fun selectFontFamily(fontFamily: AppFontFamily) {}
            },
        )
    }
}
