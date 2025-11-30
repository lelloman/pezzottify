package com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
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
                colorPalette = interactor.getColorPalette(),
                fontFamily = interactor.getFontFamily(),
            )
            mutableState.value = initialState

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
        fun getColorPalette(): ColorPalette
        fun getFontFamily(): AppFontFamily
        fun observeColorPalette(): kotlinx.coroutines.flow.Flow<ColorPalette>
        fun observeFontFamily(): kotlinx.coroutines.flow.Flow<AppFontFamily>
        suspend fun setColorPalette(colorPalette: ColorPalette)
        suspend fun setFontFamily(fontFamily: AppFontFamily)
    }
}
