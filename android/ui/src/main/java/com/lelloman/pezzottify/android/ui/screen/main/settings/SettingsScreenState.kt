package com.lelloman.pezzottify.android.ui.screen.main.settings

import com.lelloman.pezzottify.android.ui.model.PlayBehavior
import com.lelloman.pezzottify.android.ui.model.StorageInfo
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode

data class SettingsScreenState(
    val playBehavior: PlayBehavior = PlayBehavior.Default,
    val themeMode: ThemeMode = ThemeMode.Default,
    val colorPalette: ColorPalette = ColorPalette.Default,
    val fontFamily: AppFontFamily = AppFontFamily.Default,
    val isCacheEnabled: Boolean = true,
    val storageInfo: StorageInfo? = null,
    val directDownloadsEnabled: Boolean = false,
    val hasIssueContentDownloadPermission: Boolean = false,
    val isFileLoggingEnabled: Boolean = false,
    val hasLogFiles: Boolean = false,
    val logFilesSize: String = "",
)
