package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.annotation.StringRes
import com.lelloman.pezzottify.android.domain.settings.BackgroundSyncInterval
import com.lelloman.pezzottify.android.ui.model.StorageInfo
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode

data class SettingsScreenState(
    val themeMode: ThemeMode = ThemeMode.Default,
    val colorPalette: ColorPalette = ColorPalette.Default,
    val fontFamily: AppFontFamily = AppFontFamily.Default,
    val isCacheEnabled: Boolean = true,
    val storageInfo: StorageInfo? = null,
    val notifyWhatsNewEnabled: Boolean = false,
    val backgroundSyncInterval: BackgroundSyncInterval = BackgroundSyncInterval.Default,
    val smartSearchEnabled: Boolean = true,
    val excludeUnavailableEnabled: Boolean = true,
    val isFileLoggingEnabled: Boolean = false,
    val hasLogFiles: Boolean = false,
    val logFilesSize: String = "",
    val baseUrl: String = "",
    val baseUrlInput: String = "",
    @StringRes val baseUrlErrorRes: Int? = null,
    val isBaseUrlSaving: Boolean = false,
    val canReportBug: Boolean = false,
    // Cache management
    val staticsCacheSizeBytes: Long? = null,
    val imageCacheSizeBytes: Long? = null,
    val isTrimStaticsInProgress: Boolean = false,
    val isTrimImageInProgress: Boolean = false,
    val isClearStaticsInProgress: Boolean = false,
    val isClearImageInProgress: Boolean = false,
)
