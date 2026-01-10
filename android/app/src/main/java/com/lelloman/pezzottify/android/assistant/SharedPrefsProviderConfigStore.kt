package com.lelloman.pezzottify.android.assistant

import android.content.Context
import android.content.SharedPreferences
import com.lelloman.simpleaiassistant.llm.ProviderConfigStore
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * SharedPreferences-based implementation of [ProviderConfigStore].
 */
class SharedPrefsProviderConfigStore(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO
) : ProviderConfigStore {

    private val prefs: SharedPreferences = context.getSharedPreferences(
        PREFS_NAME, Context.MODE_PRIVATE
    )

    private val _selectedProviderId = MutableStateFlow(loadProviderId())
    override val selectedProviderId: StateFlow<String?> = _selectedProviderId.asStateFlow()

    private val _config = MutableStateFlow(loadConfig())
    override val config: StateFlow<Map<String, Any?>> = _config.asStateFlow()

    override suspend fun save(providerId: String, config: Map<String, Any?>) {
        withContext(dispatcher) {
            val configJson = encodeConfig(config)
            prefs.edit()
                .putString(KEY_PROVIDER_ID, providerId)
                .putString(KEY_CONFIG, configJson)
                .apply()

            _selectedProviderId.value = providerId
            _config.value = config
        }
    }

    override suspend fun clear() {
        withContext(dispatcher) {
            prefs.edit()
                .remove(KEY_PROVIDER_ID)
                .remove(KEY_CONFIG)
                .apply()

            _selectedProviderId.value = null
            _config.value = emptyMap()
        }
    }

    private fun loadProviderId(): String? {
        return prefs.getString(KEY_PROVIDER_ID, null)
    }

    private fun loadConfig(): Map<String, Any?> {
        val configJson = prefs.getString(KEY_CONFIG, null) ?: return emptyMap()
        return try {
            decodeConfig(configJson)
        } catch (e: Exception) {
            emptyMap()
        }
    }

    private fun encodeConfig(config: Map<String, Any?>): String {
        val jsonObject = JSONObject()
        config.forEach { (key, value) ->
            jsonObject.put(key, value ?: JSONObject.NULL)
        }
        return jsonObject.toString()
    }

    private fun decodeConfig(configJson: String): Map<String, Any?> {
        val jsonObject = JSONObject(configJson)
        val result = mutableMapOf<String, Any?>()
        jsonObject.keys().forEach { key ->
            val value = jsonObject.get(key)
            result[key] = if (value == JSONObject.NULL) null else value
        }
        return result
    }

    companion object {
        private const val PREFS_NAME = "assistant_provider_config"
        private const val KEY_PROVIDER_ID = "provider_id"
        private const val KEY_CONFIG = "config"
    }
}
