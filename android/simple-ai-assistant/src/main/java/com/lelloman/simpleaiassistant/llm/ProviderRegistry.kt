package com.lelloman.simpleaiassistant.llm

/**
 * Registry of available LLM provider factories.
 *
 * Populated manually by the app during initialization with the provider
 * factories for the provider modules included in the build.
 */
class ProviderRegistry(
    private val factories: List<LlmProviderFactory>
) {
    constructor(vararg factories: LlmProviderFactory) : this(factories.toList())

    /**
     * Get all available provider factories.
     */
    fun getFactories(): List<LlmProviderFactory> = factories

    /**
     * Get a provider factory by its ID.
     */
    fun getFactory(providerId: String): LlmProviderFactory? =
        factories.find { it.providerId == providerId }

    /**
     * Get provider IDs.
     */
    fun getProviderIds(): List<String> = factories.map { it.providerId }

    /**
     * Check if only one provider is available.
     * When true, the settings UI can skip the provider picker.
     */
    fun isSingleProvider(): Boolean = factories.size == 1

    /**
     * Get the single provider factory if only one is available.
     */
    fun getSingleFactory(): LlmProviderFactory? =
        if (isSingleProvider()) factories.firstOrNull() else null
}
