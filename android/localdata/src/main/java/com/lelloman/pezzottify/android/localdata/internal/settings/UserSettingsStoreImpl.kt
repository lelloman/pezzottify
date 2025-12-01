package com.lelloman.pezzottify.android.localdata.internal.settings

import android.content.Context
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.domain.settings.PlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
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

    private val mutablePlayBehavior by lazy {
        val storedValue = prefs.getString(KEY_PLAY_BEHAVIOR, null)
        val playBehavior = storedValue?.let { parsePlayBehavior(it) } ?: PlayBehavior.Default
        MutableStateFlow(playBehavior)
    }
    override val playBehavior: StateFlow<PlayBehavior> = mutablePlayBehavior.asStateFlow()

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

    override suspend fun setPlayBehavior(playBehavior: PlayBehavior) {
        withContext(dispatcher) {
            mutablePlayBehavior.value = playBehavior
            prefs.edit().putString(KEY_PLAY_BEHAVIOR, playBehavior.name).commit()
        }
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

    private fun parsePlayBehavior(value: String): PlayBehavior? = try {
        PlayBehavior.valueOf(value)
    } catch (e: IllegalArgumentException) {
        null
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

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "UserSettingsStore"
        const val KEY_PLAY_BEHAVIOR = "PlayBehavior"
        const val KEY_THEME_MODE = "ThemeMode"
        const val KEY_COLOR_PALETTE = "ColorPalette"
        const val KEY_FONT_FAMILY = "FontFamily"
        const val KEY_IN_MEMORY_CACHE_ENABLED = "InMemoryCacheEnabled"
        const val DEFAULT_IN_MEMORY_CACHE_ENABLED = true
        // Legacy value for migration - AmoledBlack was removed and converted to Amoled theme mode
        const val LEGACY_AMOLED_BLACK_PALETTE = "AmoledBlack"
    }
}
