package com.lelloman.simpleaiassistant.provider.simpleai

import android.content.Context
import android.content.Intent
import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.llm.LlmProviderFactory
import com.lelloman.simpleaiassistant.llm.ProviderConfigSchema

/**
 * Factory for creating SimpleAI LLM provider instances.
 *
 * SimpleAI communicates with the SimpleAI Android app via AIDL.
 * No configuration is needed from the user - the SimpleAI app handles
 * all backend communication.
 *
 * @param context Application context for binding to SimpleAI service
 * @param authTokenProvider Function that returns the current auth token for cloud AI calls
 */
class SimpleAiProviderFactory(
    private val context: Context,
    private val authTokenProvider: () -> String?
) : LlmProviderFactory {

    companion object {
        private const val PACKAGE = "com.lelloman.simpleai"
        private const val SERVICE_ACTION = "com.lelloman.simpleai.SIMPLE_AI_SERVICE"
    }

    override val providerId: String = "simpleai"

    override val displayName: String = "SimpleAI"

    // No configuration needed - SimpleAI app handles everything
    override val configSchema: ProviderConfigSchema = ProviderConfigSchema(fields = emptyList())

    override fun createProvider(config: Map<String, Any?>): LlmProvider {
        val authToken = authTokenProvider() ?: ""
        return SimpleAiProvider(
            context = context,
            config = SimpleAiConfig(authToken = authToken)
        )
    }

    override fun getDefaultConfig(): Map<String, Any?> = emptyMap()

    override fun validateConfig(config: Map<String, Any?>): String? {
        // Check if SimpleAI app is installed
        if (!isInstalled()) {
            return "SimpleAI app is not installed"
        }

        // Check auth token availability
        if (authTokenProvider() == null) {
            return "Not logged in. Please log in to use SimpleAI."
        }

        return null
    }

    override suspend fun fetchDynamicOptions(fieldKey: String, config: Map<String, Any?>): List<String> {
        return emptyList()
    }

    override suspend fun testConnection(config: Map<String, Any?>): Result<Unit> {
        val validationError = validateConfig(config)
        if (validationError != null) {
            return Result.failure(IllegalArgumentException(validationError))
        }

        val provider = createProvider(config) as SimpleAiProvider
        return try {
            provider.testConnection()
        } finally {
            provider.release()
        }
    }

    private fun isInstalled(): Boolean {
        val intent = Intent(SERVICE_ACTION).apply {
            setPackage(PACKAGE)
        }
        return context.packageManager.resolveService(intent, 0) != null
    }
}
