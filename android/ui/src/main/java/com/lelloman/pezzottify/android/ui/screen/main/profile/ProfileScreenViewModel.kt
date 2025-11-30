package com.lelloman.pezzottify.android.ui.screen.main.profile

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
class ProfileScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), ProfileScreenActions {

    private val mutableState = MutableStateFlow(ProfileScreenState())
    val state: StateFlow<ProfileScreenState> = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<ProfileScreenEvents>()
    val events: SharedFlow<ProfileScreenEvents> = mutableEvents.asSharedFlow()

    init {
        viewModelScope.launch {
            val initialState = ProfileScreenState(
                userName = interactor.getUserName(),
                baseUrl = interactor.getBaseUrl(),
                playBehavior = interactor.getPlayBehavior(),
                themeMode = interactor.getThemeMode(),
                colorPalette = interactor.getColorPalette(),
                fontFamily = interactor.getFontFamily(),
                buildVariant = interactor.getBuildVariant(),
                versionName = interactor.getVersionName(),
                gitCommit = interactor.getGitCommit(),
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
        }
    }

    override fun clickOnLogout() {
        if (!mutableState.value.isLoggingOut) {
            mutableState.update { it.copy(showLogoutConfirmation = true) }
        }
    }

    override fun confirmLogout() {
        if (!mutableState.value.isLoggingOut) {
            mutableState.update { it.copy(showLogoutConfirmation = false, isLoggingOut = true) }
            viewModelScope.launch {
                interactor.logout()
                mutableState.update { it.copy(isLoggingOut = false) }
                mutableEvents.emit(ProfileScreenEvents.NavigateToLoginScreen)
            }
        }
    }

    override fun dismissLogoutConfirmation() {
        mutableState.update { it.copy(showLogoutConfirmation = false) }
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

    interface Interactor {
        suspend fun logout()
        fun getUserName(): String
        fun getBaseUrl(): String
        fun getPlayBehavior(): PlayBehavior
        fun getThemeMode(): ThemeMode
        fun getColorPalette(): ColorPalette
        fun getFontFamily(): AppFontFamily
        fun observePlayBehavior(): kotlinx.coroutines.flow.Flow<PlayBehavior>
        fun observeThemeMode(): kotlinx.coroutines.flow.Flow<ThemeMode>
        fun observeColorPalette(): kotlinx.coroutines.flow.Flow<ColorPalette>
        fun observeFontFamily(): kotlinx.coroutines.flow.Flow<AppFontFamily>
        suspend fun setPlayBehavior(playBehavior: PlayBehavior)
        suspend fun setThemeMode(themeMode: ThemeMode)
        suspend fun setColorPalette(colorPalette: ColorPalette)
        suspend fun setFontFamily(fontFamily: AppFontFamily)
        fun getBuildVariant(): String
        fun getVersionName(): String
        fun getGitCommit(): String
    }
}
