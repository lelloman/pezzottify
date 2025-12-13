package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.annotation.StringRes
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
    val externalSearchEnabled: Boolean = false,
    val hasRequestContentPermission: Boolean = false,
    val isFileLoggingEnabled: Boolean = false,
    val hasLogFiles: Boolean = false,
    val logFilesSize: String = "",
    val baseUrl: String = "",
    val baseUrlInput: String = "",
    @StringRes val baseUrlErrorRes: Int? = null,
    val isBaseUrlSaving: Boolean = false,
    val isSkeletonResyncing: Boolean = false,
    val skeletonResyncResult: SkeletonResyncResult? = null,
)

sealed interface SkeletonResyncResult {
    data object Success : SkeletonResyncResult
    data object AlreadyUpToDate : SkeletonResyncResult
    data class Failed(val error: String) : SkeletonResyncResult
}
