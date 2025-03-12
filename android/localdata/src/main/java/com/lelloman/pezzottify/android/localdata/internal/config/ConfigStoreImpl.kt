package com.lelloman.pezzottify.android.localdata.internal.config

import android.content.Context
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.localdata.DefaultHostUrl
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext

internal class ConfigStoreImpl(
    context: Context,
    @DefaultHostUrl private val defaultHostUrl: String,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : ConfigStore {

    private val prefs = context.getSharedPreferences(SHARED_PREF_FILE_NAME, Context.MODE_PRIVATE)

    private val mutableHostUrl by lazy {
        MutableStateFlow(prefs.getString(KEY_HOST_URL, defaultHostUrl).orEmpty())
    }
    override val baseUrl: StateFlow<String> = mutableHostUrl.asStateFlow()

    override suspend fun setBaseUrl(baseUrl: String) {
        withContext(dispatcher) {
            mutableHostUrl.value = baseUrl
            prefs.edit().putString(KEY_HOST_URL, baseUrl).commit()
        }
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "ConfigStore"
        private const val KEY_HOST_URL = "HostUrl"
    }
}