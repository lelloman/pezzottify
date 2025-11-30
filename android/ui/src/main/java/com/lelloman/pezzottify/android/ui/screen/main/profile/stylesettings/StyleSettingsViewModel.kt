package com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.domain.settings.ThemeMode
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class StyleSettingsViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), StyleSettingsActions {

    private val mutableState = MutableStateFlow(StyleSettingsState())
    val state: StateFlow<StyleSettingsState> = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            val initialState = StyleSettingsState(
                themeMode = interactor.getThemeMode(),
                colorPalette = interactor.getColorPalette(),
                fontFamily = interactor.getFontFamily(),
            )
            mutableState.value = initialState

            launch {
                interactor.observeThemeMode().collect { themeMode ->
                    mutableState.update { it.copy(themeMode = themeMode) }
                }
            }
            launch {
                interactor.observeColorPalette().collect { colorPalette ->
                    mutableState.update { it.copy(colorPalette = colorPalette) }
                }
            }
            launch {
                interactor.observeFontFamily().collect { fontFamily ->
                    mutableState.update { it.copy(fontFamily = fontFamily) }
                }
            }
        }
    }

    override fun selectThemeMode(themeMode: ThemeMode) {
        viewModelScope.launch {
            interactor.setThemeMode(themeMode)
        }
    }

    override fun selectColorPalette(colorPalette: ColorPalette) {
        viewModelScope.launch {
            interactor.setColorPalette(colorPalette)
        }
    }

    override fun selectFontFamily(fontFamily: AppFontFamily) {
        viewModelScope.launch {
            interactor.setFontFamily(fontFamily)
        }
    }

    interface Interactor {
        fun getThemeMode(): ThemeMode
        fun getColorPalette(): ColorPalette
        fun getFontFamily(): AppFontFamily
        fun observeThemeMode(): kotlinx.coroutines.flow.Flow<ThemeMode>
        fun observeColorPalette(): kotlinx.coroutines.flow.Flow<ColorPalette>
        fun observeFontFamily(): kotlinx.coroutines.flow.Flow<AppFontFamily>
        suspend fun setThemeMode(themeMode: ThemeMode)
        suspend fun setColorPalette(colorPalette: ColorPalette)
        suspend fun setFontFamily(fontFamily: AppFontFamily)
    }
}
