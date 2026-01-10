package com.lelloman.simpleaiassistant.provider.ollama

import com.lelloman.simpleaiassistant.llm.ConfigField
import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.llm.LlmProviderFactory
import com.lelloman.simpleaiassistant.llm.ProviderConfigSchema

/**
 * Factory for creating Ollama LLM provider instances.
 */
class OllamaProviderFactory : LlmProviderFactory {

    override val providerId: String = "ollama"

    override val displayName: String = "Ollama"

    override val configSchema: ProviderConfigSchema = ProviderConfigSchema(
        fields = listOf(
            ConfigField.Text(
                key = KEY_BASE_URL,
                label = "Server URL",
                description = "The URL of your Ollama server (use 10.0.2.2 for emulator)",
                default = DEFAULT_BASE_URL,
                placeholder = "http://10.0.2.2:11434"
            ),
            ConfigField.Select(
                key = KEY_MODEL,
                label = "Model",
                description = "The model to use for chat",
                default = DEFAULT_MODEL,
                allowCustom = true,
                dynamicOptions = true
            ),
            ConfigField.Number(
                key = KEY_TIMEOUT_MS,
                label = "Timeout",
                description = "Connection timeout in seconds",
                default = DEFAULT_TIMEOUT_MS / 1000,
                min = 10,
                max = 600,
                suffix = "seconds"
            )
        )
    )

    override fun createProvider(config: Map<String, Any?>): LlmProvider {
        val ollamaConfig = configToOllamaConfig(config)
        return OllamaProvider(ollamaConfig)
    }

    override fun getDefaultConfig(): Map<String, Any?> = mapOf(
        KEY_BASE_URL to DEFAULT_BASE_URL,
        KEY_MODEL to DEFAULT_MODEL,
        KEY_TIMEOUT_MS to DEFAULT_TIMEOUT_MS / 1000
    )

    override fun validateConfig(config: Map<String, Any?>): String? {
        val baseUrl = config[KEY_BASE_URL] as? String
        if (baseUrl.isNullOrBlank()) {
            return "Server URL is required"
        }
        if (!baseUrl.startsWith("http://") && !baseUrl.startsWith("https://")) {
            return "Server URL must start with http:// or https://"
        }

        val model = config[KEY_MODEL] as? String
        if (model.isNullOrBlank()) {
            return "Model is required"
        }

        val timeout = config[KEY_TIMEOUT_MS]
        if (timeout == null || (timeout as? Number)?.toLong()?.let { it < 1 } == true) {
            return "Timeout must be at least 1 second"
        }

        return null
    }

    override suspend fun fetchDynamicOptions(fieldKey: String, config: Map<String, Any?>): List<String> {
        if (fieldKey != KEY_MODEL) return emptyList()

        val baseUrl = config[KEY_BASE_URL] as? String ?: return emptyList()
        val tempConfig = OllamaConfig(
            baseUrl = baseUrl,
            model = "",
            timeoutMs = 10_000L // Short timeout for listing models
        )
        val provider = OllamaProvider(tempConfig)
        return provider.listModels().getOrDefault(emptyList())
    }

    override suspend fun testConnection(config: Map<String, Any?>): Result<Unit> {
        val validationError = validateConfig(config)
        if (validationError != null) {
            return Result.failure(IllegalArgumentException(validationError))
        }

        val ollamaConfig = configToOllamaConfig(config)
        val provider = OllamaProvider(ollamaConfig)
        return provider.testConnection()
    }

    private fun configToOllamaConfig(config: Map<String, Any?>): OllamaConfig {
        val baseUrl = (config[KEY_BASE_URL] as? String) ?: DEFAULT_BASE_URL
        val model = (config[KEY_MODEL] as? String) ?: DEFAULT_MODEL
        val timeoutSeconds = (config[KEY_TIMEOUT_MS] as? Number)?.toLong() ?: (DEFAULT_TIMEOUT_MS / 1000)

        return OllamaConfig(
            baseUrl = baseUrl.trimEnd('/'),
            model = model,
            timeoutMs = timeoutSeconds * 1000
        )
    }

    companion object {
        const val KEY_BASE_URL = "base_url"
        const val KEY_MODEL = "model"
        const val KEY_TIMEOUT_MS = "timeout_seconds"

        // 10.0.2.2 is the Android emulator's alias for host localhost
        const val DEFAULT_BASE_URL = "http://10.0.2.2:11434"
        const val DEFAULT_MODEL = "gpt-oss:20b"
        const val DEFAULT_TIMEOUT_MS = 120_000L
    }
}
