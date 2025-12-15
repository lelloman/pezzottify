package com.lelloman.pezzottify.android

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.core.content.ContextCompat
import coil3.ImageLoader
import coil3.compose.setSingletonImageLoaderFactory
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.ui.AppUi
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

private typealias UiThemeMode = com.lelloman.pezzottify.android.ui.theme.ThemeMode
private typealias DomainThemeMode = com.lelloman.pezzottify.android.domain.settings.ThemeMode
private typealias UiColorPalette = com.lelloman.pezzottify.android.ui.theme.ColorPalette
private typealias DomainColorPalette = com.lelloman.pezzottify.android.domain.settings.ColorPalette
private typealias UiAppFontFamily = com.lelloman.pezzottify.android.ui.theme.AppFontFamily
private typealias DomainAppFontFamily = com.lelloman.pezzottify.android.domain.settings.AppFontFamily

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject
    lateinit var imageLoader: ImageLoader

    @Inject
    lateinit var userSettingsStore: UserSettingsStore

    private val notificationPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { /* Permission result - notification will show if granted */ }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        requestNotificationPermissionIfNeeded()
        setContent {
            setSingletonImageLoaderFactory { imageLoader }
            val themeMode by userSettingsStore.themeMode.collectAsState()
            val colorPalette by userSettingsStore.colorPalette.collectAsState()
            val fontFamily by userSettingsStore.fontFamily.collectAsState()
            AppUi(
                darkTheme = isSystemInDarkTheme(),
                themeMode = themeMode.toUi(),
                colorPalette = colorPalette.toUi(),
                fontFamily = fontFamily.toUi(),
            )
        }
    }

    private fun requestNotificationPermissionIfNeeded() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val permission = Manifest.permission.POST_NOTIFICATIONS
            if (ContextCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED) {
                notificationPermissionLauncher.launch(permission)
            }
        }
    }
}

private fun DomainThemeMode.toUi(): UiThemeMode = when (this) {
    DomainThemeMode.System -> UiThemeMode.System
    DomainThemeMode.Light -> UiThemeMode.Light
    DomainThemeMode.Dark -> UiThemeMode.Dark
    DomainThemeMode.Amoled -> UiThemeMode.Amoled
}

private fun DomainColorPalette.toUi(): UiColorPalette = when (this) {
    DomainColorPalette.Classic -> UiColorPalette.Classic
    DomainColorPalette.OceanBlue -> UiColorPalette.OceanBlue
    DomainColorPalette.SunsetCoral -> UiColorPalette.SunsetCoral
    DomainColorPalette.PurpleHaze -> UiColorPalette.PurpleHaze
    DomainColorPalette.RoseGold -> UiColorPalette.RoseGold
    DomainColorPalette.Midnight -> UiColorPalette.Midnight
    DomainColorPalette.Forest -> UiColorPalette.Forest
}

private fun DomainAppFontFamily.toUi(): UiAppFontFamily = when (this) {
    DomainAppFontFamily.System -> UiAppFontFamily.System
    DomainAppFontFamily.SansSerif -> UiAppFontFamily.SansSerif
    DomainAppFontFamily.Serif -> UiAppFontFamily.Serif
    DomainAppFontFamily.Monospace -> UiAppFontFamily.Monospace
}
