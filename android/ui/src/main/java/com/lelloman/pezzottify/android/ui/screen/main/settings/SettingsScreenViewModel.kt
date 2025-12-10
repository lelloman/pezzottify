package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.model.StorageInfo
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode
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
            val baseUrl = interactor.getBaseUrl()
            val initialState = SettingsScreenState(
                themeMode = interactor.getThemeMode(),
                colorPalette = interactor.getColorPalette(),
                fontFamily = interactor.getFontFamily(),
                isCacheEnabled = interactor.isCacheEnabled(),
                storageInfo = interactor.getStorageInfo(),
                directDownloadsEnabled = interactor.isDirectDownloadsEnabled(),
                hasIssueContentDownloadPermission = interactor.hasIssueContentDownloadPermission(),
                externalSearchEnabled = interactor.isExternalSearchEnabled(),
                hasRequestContentPermission = interactor.hasRequestContentPermission(),
                isFileLoggingEnabled = interactor.isFileLoggingEnabled(),
                hasLogFiles = interactor.hasLogFiles(),
                logFilesSize = interactor.getLogFilesSize(),
                baseUrl = baseUrl,
                baseUrlInput = baseUrl,
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
            launch {
                interactor.observeDirectDownloadsEnabled().collect { enabled ->
                    mutableState.update { it.copy(directDownloadsEnabled = enabled) }
                }
            }
            launch {
                interactor.observeHasIssueContentDownloadPermission().collect { hasPermission ->
                    mutableState.update { it.copy(hasIssueContentDownloadPermission = hasPermission) }
                }
            }
            launch {
                interactor.observeExternalSearchEnabled().collect { enabled ->
                    mutableState.update { it.copy(externalSearchEnabled = enabled) }
                }
            }
            launch {
                interactor.observeHasRequestContentPermission().collect { hasPermission ->
                    mutableState.update { it.copy(hasRequestContentPermission = hasPermission) }
                }
            }
            launch {
                interactor.observeFileLoggingEnabled().collect { enabled ->
                    mutableState.update { it.copy(isFileLoggingEnabled = enabled) }
                }
            }
        }
    }

    private fun refreshLogFileState() {
        mutableState.update {
            it.copy(
                hasLogFiles = interactor.hasLogFiles(),
                logFilesSize = interactor.getLogFilesSize(),
            )
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

    override fun setDirectDownloadsEnabled(enabled: Boolean) {
        viewModelScope.launch {
            interactor.setDirectDownloadsEnabled(enabled)
        }
    }

    override fun setExternalSearchEnabled(enabled: Boolean) {
        viewModelScope.launch {
            interactor.setExternalSearchEnabled(enabled)
        }
    }

    override fun setFileLoggingEnabled(enabled: Boolean) {
        viewModelScope.launch {
            interactor.setFileLoggingEnabled(enabled)
            refreshLogFileState()
        }
    }

    override fun shareLogs() {
        val intent = interactor.getShareLogsIntent()
        viewModelScope.launch {
            mutableEvents.emit(SettingsScreenEvents.ShareLogs(intent))
        }
    }

    override fun clearLogs() {
        interactor.clearLogs()
        refreshLogFileState()
    }

    override fun onBaseUrlInputChanged(input: String) {
        mutableState.update { it.copy(baseUrlInput = input, baseUrlError = null) }
    }

    override fun saveBaseUrl() {
        val input = mutableState.value.baseUrlInput.trim()
        if (input == mutableState.value.baseUrl) {
            return
        }
        mutableState.update { it.copy(isBaseUrlSaving = true, baseUrlError = null) }
        viewModelScope.launch {
            when (interactor.setBaseUrl(input)) {
                SetBaseUrlResult.Success -> {
                    mutableState.update {
                        it.copy(
                            baseUrl = input,
                            baseUrlInput = input,
                            isBaseUrlSaving = false,
                            baseUrlError = null
                        )
                    }
                }
                SetBaseUrlResult.InvalidUrl -> {
                    mutableState.update {
                        it.copy(
                            isBaseUrlSaving = false,
                            baseUrlError = "Invalid URL"
                        )
                    }
                }
            }
        }
    }

    sealed interface SetBaseUrlResult {
        data object Success : SetBaseUrlResult
        data object InvalidUrl : SetBaseUrlResult
    }

    interface Interactor {
        fun getThemeMode(): ThemeMode
        fun getColorPalette(): ColorPalette
        fun getFontFamily(): AppFontFamily
        fun isCacheEnabled(): Boolean
        fun getStorageInfo(): StorageInfo?
        fun isDirectDownloadsEnabled(): Boolean
        fun hasIssueContentDownloadPermission(): Boolean
        fun isExternalSearchEnabled(): Boolean
        fun hasRequestContentPermission(): Boolean
        fun observeThemeMode(): kotlinx.coroutines.flow.Flow<ThemeMode>
        fun observeColorPalette(): kotlinx.coroutines.flow.Flow<ColorPalette>
        fun observeFontFamily(): kotlinx.coroutines.flow.Flow<AppFontFamily>
        fun observeCacheEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeStorageInfo(): kotlinx.coroutines.flow.Flow<StorageInfo>
        fun observeDirectDownloadsEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeHasIssueContentDownloadPermission(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeExternalSearchEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeHasRequestContentPermission(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeFileLoggingEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        suspend fun setThemeMode(themeMode: ThemeMode)
        suspend fun setColorPalette(colorPalette: ColorPalette)
        suspend fun setFontFamily(fontFamily: AppFontFamily)
        suspend fun setCacheEnabled(enabled: Boolean)
        suspend fun setDirectDownloadsEnabled(enabled: Boolean): Boolean
        suspend fun setExternalSearchEnabled(enabled: Boolean)
        suspend fun setFileLoggingEnabled(enabled: Boolean)
        fun isFileLoggingEnabled(): Boolean
        fun hasLogFiles(): Boolean
        fun getLogFilesSize(): String
        fun getShareLogsIntent(): android.content.Intent
        fun clearLogs()
        fun getBaseUrl(): String
        suspend fun setBaseUrl(url: String): SetBaseUrlResult
    }
}
