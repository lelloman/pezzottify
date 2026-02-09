package com.lelloman.pezzottify.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import coil3.ImageLoader
import coil3.compose.setSingletonImageLoaderFactory
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.mapping.toAppFontFamily
import com.lelloman.pezzottify.android.mapping.toColorPalette
import com.lelloman.pezzottify.android.ui.tv.TvAppUi
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class TvMainActivity : ComponentActivity() {

    @Inject
    lateinit var imageLoader: ImageLoader

    @Inject
    lateinit var userSettingsStore: UserSettingsStore

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        setContent {
            setSingletonImageLoaderFactory { imageLoader }
            val fontFamily by userSettingsStore.fontFamily.collectAsState()
            TvAppUi(
                darkTheme = true,
                themeMode = com.lelloman.pezzottify.android.ui.theme.ThemeMode.Dark,
                colorPalette = com.lelloman.pezzottify.android.ui.theme.ColorPalette.Classic,
                fontFamily = fontFamily.toAppFontFamily(),
            )
        }
    }
}
