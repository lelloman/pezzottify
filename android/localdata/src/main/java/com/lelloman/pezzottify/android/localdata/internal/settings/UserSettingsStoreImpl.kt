package com.lelloman.pezzottify.android.localdata.internal.settings

import android.content.Context
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

    private val mutablePlayBehavior by lazy {
        val storedValue = prefs.getString(KEY_PLAY_BEHAVIOR, null)
        val playBehavior = storedValue?.let { parsePlayBehavior(it) } ?: PlayBehavior.Default
        MutableStateFlow(playBehavior)
    }
    override val playBehavior: StateFlow<PlayBehavior> = mutablePlayBehavior.asStateFlow()

    private val mutableThemeMode by lazy {
        val storedValue = prefs.getString(KEY_THEME_MODE, null)
        val themeMode = storedValue?.let { parseThemeMode(it) } ?: ThemeMode.Default
        MutableStateFlow(themeMode)
    }
    override val themeMode: StateFlow<ThemeMode> = mutableThemeMode.asStateFlow()

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

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "UserSettingsStore"
        const val KEY_PLAY_BEHAVIOR = "PlayBehavior"
        const val KEY_THEME_MODE = "ThemeMode"
    }
}
