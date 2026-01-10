package com.lelloman.simpleaiassistant.llm

import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.StreamEvent
import com.lelloman.simpleaiassistant.tool.ToolSpec
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch

/**
 * A dynamic LLM provider that creates and manages the actual provider based on
 * the configuration stored in [ProviderConfigStore].
 *
 * When the config changes, the underlying provider is recreated automatically.
 */
class DynamicLlmProvider(
    private val registry: ProviderRegistry,
    private val configStore: ProviderConfigStore,
    scope: CoroutineScope
) : LlmProvider {

    private var currentProvider: LlmProvider? = null
    private var currentProviderId: String? = null

    private val _isConfigured = MutableStateFlow(false)
    val isConfigured: StateFlow<Boolean> = _isConfigured.asStateFlow()

    init {
        scope.launch {
            combine(
                configStore.selectedProviderId,
                configStore.config
            ) { providerId, config ->
                Pair(providerId, config)
            }.collect { (providerId, config) ->
                updateProvider(providerId, config)
            }
        }
    }

    private fun updateProvider(providerId: String?, config: Map<String, Any?>) {
        if (providerId == null) {
            // No provider selected, use the default provider
            val defaultFactory = registry.getDefaultFactory()
            if (defaultFactory != null) {
                val defaultConfig = defaultFactory.getDefaultConfig()
                currentProvider = defaultFactory.createProvider(defaultConfig)
                currentProviderId = defaultFactory.providerId
                _isConfigured.value = false // Using defaults, not user-configured
            } else {
                currentProvider = null
                currentProviderId = null
                _isConfigured.value = false
            }
            return
        }

        val factory = registry.getFactory(providerId)
        if (factory == null) {
            currentProvider = null
            currentProviderId = null
            _isConfigured.value = false
            return
        }

        // Merge with defaults to ensure all keys are present
        val fullConfig = factory.getDefaultConfig().toMutableMap()
        fullConfig.putAll(config)

        currentProvider = factory.createProvider(fullConfig)
        currentProviderId = providerId
        _isConfigured.value = true
    }

    override val id: String
        get() = currentProvider?.id ?: "none"

    override val displayName: String
        get() = currentProvider?.displayName ?: "Not Configured"

    override fun streamChat(
        messages: List<ChatMessage>,
        tools: List<ToolSpec>,
        systemPrompt: String
    ): Flow<StreamEvent> {
        val provider = currentProvider
            ?: throw IllegalStateException("No LLM provider configured. Please configure a provider in settings.")
        return provider.streamChat(messages, tools, systemPrompt)
    }

    override suspend fun testConnection(): Result<Unit> {
        val provider = currentProvider
            ?: return Result.failure(IllegalStateException("No LLM provider configured"))
        return provider.testConnection()
    }

    override suspend fun listModels(): Result<List<String>> {
        val provider = currentProvider
            ?: return Result.failure(IllegalStateException("No LLM provider configured"))
        return provider.listModels()
    }

    override suspend fun detectLanguage(text: String): String? {
        return currentProvider?.detectLanguage(text)
    }
}
