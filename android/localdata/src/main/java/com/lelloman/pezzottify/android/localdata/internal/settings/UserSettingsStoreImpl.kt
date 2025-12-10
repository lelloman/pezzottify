package com.lelloman.pezzottify.android.localdata.internal.settings

import android.content.Context
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.domain.settings.SyncedUserSetting
import com.lelloman.pezzottify.android.domain.settings.ThemeMode
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext

internal class UserSettingsStoreImpl(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : UserSettingsStore {

    private val prefs = context.getSharedPreferences(SHARED_PREF_FILE_NAME, Context.MODE_PRIVATE)

    // Migration: if user had AmoledBlack palette (now removed), migrate to Amoled theme mode
    private val shouldMigrateToAmoledTheme: Boolean by lazy {
        val storedPalette = prefs.getString(KEY_COLOR_PALETTE, null)
        storedPalette == LEGACY_AMOLED_BLACK_PALETTE
    }

    private val mutableThemeMode by lazy {
        val storedValue = prefs.getString(KEY_THEME_MODE, null)
        val themeMode = if (shouldMigrateToAmoledTheme) {
            // Migrate user from AmoledBlack palette to Amoled theme mode
            prefs.edit().putString(KEY_THEME_MODE, ThemeMode.Amoled.name).apply()
            ThemeMode.Amoled
        } else {
            storedValue?.let { parseThemeMode(it) } ?: ThemeMode.Default
        }
        MutableStateFlow(themeMode)
    }
    override val themeMode: StateFlow<ThemeMode> = mutableThemeMode.asStateFlow()

    private val mutableColorPalette by lazy {
        val storedValue = prefs.getString(KEY_COLOR_PALETTE, null)
        val colorPalette = if (shouldMigrateToAmoledTheme) {
            // Migrate user from AmoledBlack palette to Classic palette
            prefs.edit().putString(KEY_COLOR_PALETTE, ColorPalette.Classic.name).apply()
            ColorPalette.Classic
        } else {
            storedValue?.let { parseColorPalette(it) } ?: ColorPalette.Default
        }
        MutableStateFlow(colorPalette)
    }
    override val colorPalette: StateFlow<ColorPalette> = mutableColorPalette.asStateFlow()

    private val mutableFontFamily by lazy {
        val storedValue = prefs.getString(KEY_FONT_FAMILY, null)
        val fontFamily = storedValue?.let { parseFontFamily(it) } ?: AppFontFamily.Default
        MutableStateFlow(fontFamily)
    }
    override val fontFamily: StateFlow<AppFontFamily> = mutableFontFamily.asStateFlow()

    private val mutableInMemoryCacheEnabled by lazy {
        val enabled = prefs.getBoolean(KEY_IN_MEMORY_CACHE_ENABLED, DEFAULT_IN_MEMORY_CACHE_ENABLED)
        MutableStateFlow(enabled)
    }
    override val isInMemoryCacheEnabled: StateFlow<Boolean> = mutableInMemoryCacheEnabled.asStateFlow()

    private val mutableFileLoggingEnabled by lazy {
        val enabled = prefs.getBoolean(KEY_FILE_LOGGING_ENABLED, DEFAULT_FILE_LOGGING_ENABLED)
        MutableStateFlow(enabled)
    }
    override val isFileLoggingEnabled: StateFlow<Boolean> = mutableFileLoggingEnabled.asStateFlow()

    private val mutableDirectDownloadsEnabled by lazy {
        val enabled = prefs.getBoolean(KEY_DIRECT_DOWNLOADS_ENABLED, DEFAULT_DIRECT_DOWNLOADS_ENABLED)
        MutableStateFlow(enabled)
    }
    override val directDownloadsEnabled: StateFlow<Boolean> = mutableDirectDownloadsEnabled.asStateFlow()

    // Track sync status for synced settings (key -> SyncedUserSetting)
    private val mutableSyncedSettings by lazy {
        val settings = mutableMapOf<String, SyncedUserSetting>()
        // Load existing synced settings with their sync status
        val directDownloadsStatus = prefs.getString(KEY_DIRECT_DOWNLOADS_SYNC_STATUS, null)
            ?.let { parseSyncStatus(it) }
        if (directDownloadsStatus != null && directDownloadsStatus != SyncStatus.Synced) {
            val enabled = prefs.getBoolean(KEY_DIRECT_DOWNLOADS_ENABLED, DEFAULT_DIRECT_DOWNLOADS_ENABLED)
            val modifiedAt = prefs.getLong(KEY_DIRECT_DOWNLOADS_MODIFIED_AT, System.currentTimeMillis())
            settings[KEY_SETTING_DIRECT_DOWNLOADS] = SyncedUserSetting(
                setting = UserSetting.DirectDownloadsEnabled(enabled),
                modifiedAt = modifiedAt,
                syncStatus = directDownloadsStatus,
            )
        }
        MutableStateFlow(settings.toMap())
    }

    override suspend fun setThemeMode(themeMode: ThemeMode) {
        withContext(dispatcher) {
            mutableThemeMode.value = themeMode
            prefs.edit().putString(KEY_THEME_MODE, themeMode.name).commit()
        }
    }

    override suspend fun setColorPalette(colorPalette: ColorPalette) {
        withContext(dispatcher) {
            mutableColorPalette.value = colorPalette
            prefs.edit().putString(KEY_COLOR_PALETTE, colorPalette.name).commit()
        }
    }

    override suspend fun setFontFamily(fontFamily: AppFontFamily) {
        withContext(dispatcher) {
            mutableFontFamily.value = fontFamily
            prefs.edit().putString(KEY_FONT_FAMILY, fontFamily.name).commit()
        }
    }

    override suspend fun setInMemoryCacheEnabled(enabled: Boolean) {
        withContext(dispatcher) {
            mutableInMemoryCacheEnabled.value = enabled
            prefs.edit().putBoolean(KEY_IN_MEMORY_CACHE_ENABLED, enabled).commit()
        }
    }

    override suspend fun setFileLoggingEnabled(enabled: Boolean) {
        withContext(dispatcher) {
            mutableFileLoggingEnabled.value = enabled
            prefs.edit().putBoolean(KEY_FILE_LOGGING_ENABLED, enabled).commit()
        }
    }

    override suspend fun setDirectDownloadsEnabled(enabled: Boolean) {
        withContext(dispatcher) {
            mutableDirectDownloadsEnabled.value = enabled
            prefs.edit()
                .putBoolean(KEY_DIRECT_DOWNLOADS_ENABLED, enabled)
                .putString(KEY_DIRECT_DOWNLOADS_SYNC_STATUS, SyncStatus.Synced.name)
                .remove(KEY_DIRECT_DOWNLOADS_MODIFIED_AT)
                .commit()
            // Remove from pending sync since it came from server
            val updatedSettings = mutableSyncedSettings.value.toMutableMap()
            updatedSettings.remove(KEY_SETTING_DIRECT_DOWNLOADS)
            mutableSyncedSettings.value = updatedSettings
        }
    }

    override suspend fun setSyncedSetting(setting: UserSetting, syncStatus: SyncStatus) {
        withContext(dispatcher) {
            when (setting) {
                is UserSetting.DirectDownloadsEnabled -> {
                    val enabled = setting.value
                    val modifiedAt = System.currentTimeMillis()
                    mutableDirectDownloadsEnabled.value = enabled
                    prefs.edit()
                        .putBoolean(KEY_DIRECT_DOWNLOADS_ENABLED, enabled)
                        .putString(KEY_DIRECT_DOWNLOADS_SYNC_STATUS, syncStatus.name)
                        .putLong(KEY_DIRECT_DOWNLOADS_MODIFIED_AT, modifiedAt)
                        .commit()

                    val updatedSettings = mutableSyncedSettings.value.toMutableMap()
                    if (syncStatus == SyncStatus.Synced) {
                        updatedSettings.remove(KEY_SETTING_DIRECT_DOWNLOADS)
                    } else {
                        updatedSettings[KEY_SETTING_DIRECT_DOWNLOADS] = SyncedUserSetting(
                            setting = setting,
                            modifiedAt = modifiedAt,
                            syncStatus = syncStatus,
                        )
                    }
                    mutableSyncedSettings.value = updatedSettings
                }
            }
        }
    }

    override fun getPendingSyncSettings(): Flow<List<SyncedUserSetting>> {
        return mutableSyncedSettings.map { settings ->
            settings.values.filter { it.syncStatus == SyncStatus.PendingSync }
        }
    }

    override suspend fun updateSyncStatus(settingKey: String, status: SyncStatus) {
        withContext(dispatcher) {
            when (settingKey) {
                KEY_SETTING_DIRECT_DOWNLOADS -> {
                    prefs.edit()
                        .putString(KEY_DIRECT_DOWNLOADS_SYNC_STATUS, status.name)
                        .commit()

                    val updatedSettings = mutableSyncedSettings.value.toMutableMap()
                    val existing = updatedSettings[settingKey]
                    if (existing != null) {
                        if (status == SyncStatus.Synced) {
                            updatedSettings.remove(settingKey)
                        } else {
                            updatedSettings[settingKey] = existing.copy(syncStatus = status)
                        }
                        mutableSyncedSettings.value = updatedSettings
                    }
                }
            }
        }
    }

    override suspend fun clearSyncedSettings() {
        withContext(dispatcher) {
            mutableDirectDownloadsEnabled.value = DEFAULT_DIRECT_DOWNLOADS_ENABLED
            prefs.edit()
                .remove(KEY_DIRECT_DOWNLOADS_ENABLED)
                .remove(KEY_DIRECT_DOWNLOADS_SYNC_STATUS)
                .remove(KEY_DIRECT_DOWNLOADS_MODIFIED_AT)
                .commit()
            mutableSyncedSettings.value = emptyMap()
        }
    }

    private fun parseThemeMode(value: String): ThemeMode? = try {
        ThemeMode.valueOf(value)
    } catch (e: IllegalArgumentException) {
        null
    }

    private fun parseColorPalette(value: String): ColorPalette? = try {
        ColorPalette.valueOf(value)
    } catch (e: IllegalArgumentException) {
        null
    }

    private fun parseFontFamily(value: String): AppFontFamily? = try {
        AppFontFamily.valueOf(value)
    } catch (e: IllegalArgumentException) {
        null
    }

    private fun parseSyncStatus(value: String): SyncStatus? = try {
        SyncStatus.valueOf(value)
    } catch (e: IllegalArgumentException) {
        null
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "UserSettingsStore"
        const val KEY_THEME_MODE = "ThemeMode"
        const val KEY_COLOR_PALETTE = "ColorPalette"
        const val KEY_FONT_FAMILY = "FontFamily"
        const val KEY_IN_MEMORY_CACHE_ENABLED = "InMemoryCacheEnabled"
        const val DEFAULT_IN_MEMORY_CACHE_ENABLED = true
        const val KEY_FILE_LOGGING_ENABLED = "FileLoggingEnabled"
        const val DEFAULT_FILE_LOGGING_ENABLED = false
        const val KEY_DIRECT_DOWNLOADS_ENABLED = "DirectDownloadsEnabled"
        const val KEY_DIRECT_DOWNLOADS_SYNC_STATUS = "DirectDownloadsSyncStatus"
        const val KEY_DIRECT_DOWNLOADS_MODIFIED_AT = "DirectDownloadsModifiedAt"
        const val DEFAULT_DIRECT_DOWNLOADS_ENABLED = false
        // Setting key for synced settings map
        const val KEY_SETTING_DIRECT_DOWNLOADS = "enable_direct_downloads"
        // Legacy value for migration - AmoledBlack was removed and converted to Amoled theme mode
        const val LEGACY_AMOLED_BLACK_PALETTE = "AmoledBlack"
    }
}
