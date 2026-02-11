package com.lelloman.pezzottify.android.domain.settings

import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

interface UserSettingsStore {

    val themeMode: StateFlow<ThemeMode>
    val colorPalette: StateFlow<ColorPalette>
    val fontFamily: StateFlow<AppFontFamily>
    val isInMemoryCacheEnabled: StateFlow<Boolean>
    val isFileLoggingEnabled: StateFlow<Boolean>

    /**
     * Whether to receive notifications when new content batches are closed.
     * This setting is synced with the server.
     */
    val isNotifyWhatsNewEnabled: StateFlow<Boolean>

    /**
     * How often to run background sync when the app is not running.
     * This is a local-only setting.
     */
    val backgroundSyncInterval: StateFlow<BackgroundSyncInterval>

    /**
     * Whether to use smart (streaming) search instead of classic flat search.
     * This is a local-only setting.
     */
    val isSmartSearchEnabled: StateFlow<Boolean>

    /**
     * Whether to exclude unavailable content from search results.
     * This is a local-only setting.
     */
    val isExcludeUnavailableEnabled: StateFlow<Boolean>

    suspend fun setThemeMode(themeMode: ThemeMode)
    suspend fun setColorPalette(colorPalette: ColorPalette)
    suspend fun setFontFamily(fontFamily: AppFontFamily)
    suspend fun setInMemoryCacheEnabled(enabled: Boolean)
    suspend fun setFileLoggingEnabled(enabled: Boolean)
    suspend fun setNotifyWhatsNewEnabled(enabled: Boolean)
    fun setBackgroundSyncInterval(interval: BackgroundSyncInterval)
    fun setSmartSearchEnabled(enabled: Boolean)
    fun setExcludeUnavailableEnabled(enabled: Boolean)

    /**
     * Set a synced setting with specified sync status.
     * Used for local changes that need to be synced (PendingSync) or
     * when applying server state (Synced).
     */
    suspend fun setSyncedSetting(setting: UserSetting, syncStatus: SyncStatus)

    /**
     * Get all settings that are pending sync with the server.
     */
    fun getPendingSyncSettings(): Flow<List<SyncedUserSetting>>

    /**
     * Update the sync status of a setting.
     */
    suspend fun updateSyncStatus(settingKey: String, status: SyncStatus)

    /**
     * Clear all synced user settings.
     * Called on logout.
     */
    suspend fun clearSyncedSettings()
}

enum class ThemeMode {
    System,
    Light,
    Dark,
    Amoled;  // True black for OLED screens

    companion object {
        val Default = Dark
    }
}

enum class ColorPalette {
    Classic,      // Original green theme
    OceanBlue,    // Cool blue tones
    SunsetCoral,  // Warm coral/orange tones
    PurpleHaze,   // Violet/purple theme
    RoseGold,     // Warm pink/rose tones
    Midnight,     // Deep navy/indigo tones
    Forest;       // Earthy green/brown tones

    companion object {
        val Default = Classic
    }
}

enum class AppFontFamily {
    System,       // Default system fonts
    SansSerif,    // Clean sans-serif
    Serif,        // Classic serif style
    Monospace;    // Monospace for a developer feel

    companion object {
        val Default = SansSerif
    }
}

enum class BackgroundSyncInterval(val minutes: Long) {
    Minutes15(15),
    Minutes30(30),
    Hours1(60),
    Hours2(120),
    Hours4(240),
    Hours6(360),
    Hours12(720),
    Hours24(1440),
    Disabled(-1);

    companion object {
        val Default = Hours12
    }
}
