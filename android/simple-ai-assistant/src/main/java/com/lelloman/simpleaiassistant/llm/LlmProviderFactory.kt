package com.lelloman.simpleaiassistant.llm

/**
 * Factory for creating LLM provider instances.
 *
 * Each provider module (e.g., simple-ai-provider-ollama) implements this interface
 * to allow dynamic provider configuration and instantiation.
 */
interface LlmProviderFactory {

    /**
     * Unique identifier for this provider (e.g., "ollama", "anthropic", "openai")
     */
    val providerId: String

    /**
     * Human-readable name for display in UI (e.g., "Ollama", "Anthropic", "OpenAI")
     */
    val displayName: String

    /**
     * Schema describing the configuration fields for this provider.
     * Used to generate settings UI dynamically.
     */
    val configSchema: ProviderConfigSchema

    /**
     * Create a provider instance with the given configuration.
     *
     * @param config Map of configuration values, keys match [configSchema] field keys
     * @return Configured LLM provider instance
     */
    fun createProvider(config: Map<String, Any?>): LlmProvider

    /**
     * Get default configuration values for this provider.
     * Keys match [configSchema] field keys.
     */
    fun getDefaultConfig(): Map<String, Any?>

    /**
     * Validate configuration values.
     *
     * @param config Configuration to validate
     * @return null if valid, or error message if invalid
     */
    fun validateConfig(config: Map<String, Any?>): String? = null

    /**
     * Fetch dynamic options for a Select field (e.g., available models).
     * Only called for fields with [ConfigField.Select.dynamicOptions] = true.
     *
     * @param fieldKey The field key to fetch options for
     * @param config Current configuration (may be needed to connect to server)
     * @return List of available options, or empty list if fetch fails
     */
    suspend fun fetchDynamicOptions(fieldKey: String, config: Map<String, Any?>): List<String> = emptyList()

    /**
     * Test connection to the provider with the given configuration.
     *
     * @param config Configuration to test
     * @return Result indicating success or failure with error message
     */
    suspend fun testConnection(config: Map<String, Any?>): Result<Unit>
}
