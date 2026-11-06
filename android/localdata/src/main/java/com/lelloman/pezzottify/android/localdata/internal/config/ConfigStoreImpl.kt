package com.lelloman.pezzottify.android.localdata.internal.config

import android.content.Context
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore.SetBaseUrlResult
import com.lelloman.pezzottify.android.localdata.DefaultHostUrl
import com.lelloman.pezzottify.android.localdata.ServerConfigurable
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import java.net.URI

internal class ConfigStoreImpl(
    context: Context,
    @DefaultHostUrl private val defaultHostUrl: String,
    @ServerConfigurable private val serverConfigurable: Boolean = true,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : ConfigStore {

    private val prefs = context.getSharedPreferences(SHARED_PREF_FILE_NAME, Context.MODE_PRIVATE)

    private val mutableHostUrl by lazy {
        // A pinned build always talks to its server, whatever an older build saved.
        val saved = if (serverConfigurable) prefs.getString(KEY_HOST_URL, defaultHostUrl) else null
        MutableStateFlow((saved ?: defaultHostUrl).orEmpty())
    }
    override val baseUrl: StateFlow<String> = mutableHostUrl.asStateFlow()

    override suspend fun setBaseUrl(baseUrl: String): SetBaseUrlResult {
        // Pinned: nothing to change, and nothing for callers to report.
        if (!serverConfigurable) return SetBaseUrlResult.Success
        if (!isValidHttpUrl(baseUrl)) {
            return SetBaseUrlResult.InvalidUrl
        }
        withContext(dispatcher) {
            mutableHostUrl.value = baseUrl
            prefs.edit().putString(KEY_HOST_URL, baseUrl).commit()
        }
        return SetBaseUrlResult.Success
    }

    private fun isValidHttpUrl(url: String): Boolean {
        if (url.isBlank()) return false
        return try {
            val uri = URI(url)
            val scheme = uri.scheme?.lowercase()
            (scheme == "http" || scheme == "https") && !uri.host.isNullOrBlank()
        } catch (_: Exception) {
            false
        }
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "ConfigStore"
        private const val KEY_HOST_URL = "HostUrl"
    }
}