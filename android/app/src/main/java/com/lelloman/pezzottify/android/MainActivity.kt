package com.lelloman.pezzottify.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import coil3.ImageLoader
import coil3.compose.setSingletonImageLoaderFactory
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.oidc.OidcCallbackHandler
import com.lelloman.pezzottify.android.ui.AppUi
import com.lelloman.pezzottify.android.mapping.toAppFontFamily
import com.lelloman.pezzottify.android.mapping.toColorPalette as toColorPalette
import com.lelloman.pezzottify.android.mapping.toThemeMode as toThemeMode
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
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

    @Inject
    lateinit var oidcCallbackHandler: OidcCallbackHandler

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Check for OIDC callback in initial intent
        handleOidcCallback(intent)

        setContent {
            setSingletonImageLoaderFactory { imageLoader }
            val themeMode by userSettingsStore.themeMode.collectAsState()
            val colorPalette by userSettingsStore.colorPalette.collectAsState()
            val fontFamily by userSettingsStore.fontFamily.collectAsState()
            AppUi(
                darkTheme = isSystemInDarkTheme(),
                themeMode = themeMode.toThemeMode(),
                colorPalette = colorPalette.toColorPalette(),
                fontFamily = fontFamily.toAppFontFamily(),
            )
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleOidcCallback(intent)
    }

    private fun handleOidcCallback(intent: Intent) {
        if (oidcCallbackHandler.isOidcCallback(intent)) {
            oidcCallbackHandler.handleCallback(intent)
        }
    }
}
