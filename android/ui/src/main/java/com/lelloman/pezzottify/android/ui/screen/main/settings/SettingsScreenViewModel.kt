package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.R
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
                notifyWhatsNewEnabled = interactor.isNotifyWhatsNewEnabled(),
                smartSearchEnabled = interactor.isSmartSearchEnabled(),
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
                interactor.observeNotifyWhatsNewEnabled().collect { enabled ->
                    mutableState.update { it.copy(notifyWhatsNewEnabled = enabled) }
                }
            }
            launch {
                interactor.observeSmartSearchEnabled().collect { enabled ->
                    mutableState.update { it.copy(smartSearchEnabled = enabled) }
                }
            }
            launch {
                interactor.observeFileLoggingEnabled().collect { enabled ->
                    mutableState.update { it.copy(isFileLoggingEnabled = enabled) }
                }
            }
            launch {
                interactor.observeCanReportBug().collect { canReportBug ->
                    mutableState.update { it.copy(canReportBug = canReportBug) }
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

    override fun setNotifyWhatsNewEnabled(enabled: Boolean) {
        viewModelScope.launch {
            interactor.setNotifyWhatsNewEnabled(enabled)
        }
    }

    override fun setSmartSearchEnabled(enabled: Boolean) {
        interactor.setSmartSearchEnabled(enabled)
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
        mutableState.update { it.copy(baseUrlInput = input, baseUrlErrorRes = null) }
    }

    override fun saveBaseUrl() {
        val input = mutableState.value.baseUrlInput.trim()
        if (input == mutableState.value.baseUrl) {
            return
        }
        mutableState.update { it.copy(isBaseUrlSaving = true, baseUrlErrorRes = null) }
        viewModelScope.launch {
            when (interactor.setBaseUrl(input)) {
                SetBaseUrlResult.Success -> {
                    mutableState.update {
                        it.copy(
                            baseUrl = input,
                            baseUrlInput = input,
                            isBaseUrlSaving = false,
                            baseUrlErrorRes = null
                        )
                    }
                }
                SetBaseUrlResult.InvalidUrl -> {
                    mutableState.update {
                        it.copy(
                            isBaseUrlSaving = false,
                            baseUrlErrorRes = R.string.invalid_url
                        )
                    }
                }
            }
        }
    }

    override fun forceSkeletonResync() {
        if (mutableState.value.isSkeletonResyncing) {
            return
        }
        mutableState.update { it.copy(isSkeletonResyncing = true, skeletonResyncResult = null) }
        viewModelScope.launch {
            val result = interactor.forceSkeletonResync()
            mutableState.update {
                it.copy(
                    isSkeletonResyncing = false,
                    skeletonResyncResult = result
                )
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
        fun isNotifyWhatsNewEnabled(): Boolean
        fun isSmartSearchEnabled(): Boolean
        fun observeThemeMode(): kotlinx.coroutines.flow.Flow<ThemeMode>
        fun observeColorPalette(): kotlinx.coroutines.flow.Flow<ColorPalette>
        fun observeFontFamily(): kotlinx.coroutines.flow.Flow<AppFontFamily>
        fun observeCacheEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeStorageInfo(): kotlinx.coroutines.flow.Flow<StorageInfo>
        fun observeNotifyWhatsNewEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeSmartSearchEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        fun observeFileLoggingEnabled(): kotlinx.coroutines.flow.Flow<Boolean>
        suspend fun setThemeMode(themeMode: ThemeMode)
        suspend fun setColorPalette(colorPalette: ColorPalette)
        suspend fun setFontFamily(fontFamily: AppFontFamily)
        suspend fun setCacheEnabled(enabled: Boolean)
        suspend fun setNotifyWhatsNewEnabled(enabled: Boolean)
        fun setSmartSearchEnabled(enabled: Boolean)
        suspend fun setFileLoggingEnabled(enabled: Boolean)
        fun isFileLoggingEnabled(): Boolean
        fun hasLogFiles(): Boolean
        fun getLogFilesSize(): String
        fun getShareLogsIntent(): android.content.Intent
        fun clearLogs()
        fun getBaseUrl(): String
        suspend fun setBaseUrl(url: String): SetBaseUrlResult
        suspend fun forceSkeletonResync(): SkeletonResyncResult
        fun observeCanReportBug(): kotlinx.coroutines.flow.Flow<Boolean>
    }
}
