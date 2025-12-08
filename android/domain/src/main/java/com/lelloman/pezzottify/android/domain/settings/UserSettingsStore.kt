package com.lelloman.pezzottify.android.domain.settings

import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

interface UserSettingsStore {

    val playBehavior: StateFlow<PlayBehavior>
    val themeMode: StateFlow<ThemeMode>
    val colorPalette: StateFlow<ColorPalette>
    val fontFamily: StateFlow<AppFontFamily>
    val isInMemoryCacheEnabled: StateFlow<Boolean>
    val isFileLoggingEnabled: StateFlow<Boolean>

    /**
     * Whether direct downloads are enabled.
     * This setting is synced with the server and is only visible to users with IssueContentDownload permission.
     */
    val directDownloadsEnabled: StateFlow<Boolean>

    suspend fun setPlayBehavior(playBehavior: PlayBehavior)
    suspend fun setThemeMode(themeMode: ThemeMode)
    suspend fun setColorPalette(colorPalette: ColorPalette)
    suspend fun setFontFamily(fontFamily: AppFontFamily)
    suspend fun setInMemoryCacheEnabled(enabled: Boolean)
    suspend fun setFileLoggingEnabled(enabled: Boolean)

    /**
     * Set whether direct downloads are enabled.
     * This is called when receiving a sync event from the server, not directly by the user.
     * Uses Synced status since it comes from the server.
     */
    suspend fun setDirectDownloadsEnabled(enabled: Boolean)

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

enum class PlayBehavior {
    ReplacePlaylist,
    AddToPlaylist;

    companion object {
        val Default = ReplacePlaylist
    }
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
