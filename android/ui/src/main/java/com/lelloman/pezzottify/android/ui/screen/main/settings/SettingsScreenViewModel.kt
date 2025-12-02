package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.domain.settings.PlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SettingsScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), SettingsScreenActions {

    private val mutableState = MutableStateFlow(SettingsScreenState())
    val state: StateFlow<SettingsScreenState> = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<SettingsScreenEvents>()
    val events: SharedFlow<SettingsScreenEvents> = mutableEvents.asSharedFlow()

    init {
        viewModelScope.launch {
            val initialState = SettingsScreenState(
                playBehavior = interactor.getPlayBehavior(),
                themeMode = interactor.getThemeMode(),
                colorPalette = interactor.getColorPalette(),
                fontFamily = interactor.getFontFamily(),
                isCacheEnabled = interactor.isCacheEnabled(),
                storageInfo = interactor.getStorageInfo(),
            )
            mutableState.value = initialState

            launch {
                interactor.observePlayBehavior().collect { playBehavior ->
                    mutableState.update { it.copy(playBehavior = playBehavior) }
                }
            }
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
            launch {
                interactor.observeCacheEnabled().collect { enabled ->
                    mutableState.update { it.copy(isCacheEnabled = enabled) }
                }
            }
            launch {
                interactor.observeStorageInfo().collect { storageInfo ->
                    mutableState.update { it.copy(storageInfo = storageInfo) }
                }
            }
        }
    }

    override fun selectPlayBehavior(playBehavior: PlayBehavior) {
        viewModelScope.launch {
            interactor.setPlayBehavior(playBehavior)
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

    override fun setCacheEnabled(enabled: Boolean) {
        viewModelScope.launch {
            interactor.setCacheEnabled(enabled)
        }
    }

    interface Interactor {
        fun getPlayBehavior(): PlayBehavior
        fun getThemeMode(): ThemeMode
        fun getColorPalette(): ColorPalette
        fun getFontFamily(): AppFontFamily
        fun isCacheEnabled(): Boolean
        fun getStorageInfo(): com.lelloman.pezzottify.android.domain.storage.StorageInfo?
        fun observePlayBehavior(): kotlinx.coroutines.flow.Flow<PlayBehavior>
        fun observeThemeMode(): kotlinx.coroutines.flow.Flow<ThemeMode>
        fun observeColorPalette(): kotlinx.coroutines.flow.Flow<ColorPalette>
        fun observeFontFamily(): kotlinx.coroutines.flow.Flow<AppFontFamily>
        fun observeCacheEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeStorageInfo(): kotlinx.coroutines.flow.Flow<com.lelloman.pezzottify.android.domain.storage.StorageInfo>
        suspend fun setPlayBehavior(playBehavior: PlayBehavior)
        suspend fun setThemeMode(themeMode: ThemeMode)
        suspend fun setColorPalette(colorPalette: ColorPalette)
        suspend fun setFontFamily(fontFamily: AppFontFamily)
        suspend fun setCacheEnabled(enabled: Boolean)
    }
}
