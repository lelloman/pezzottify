package com.lelloman.simpleaiassistant.llm

/**
 * Registry of available LLM provider factories.
 *
 * Populated manually by the app during initialization with the provider
 * factories for the provider modules included in the build.
 *
 * @param factories List of available provider factories
 * @param defaultProviderId The provider ID to use as default when no provider is selected.
 *                          If null, uses the first provider in the list.
 */
class ProviderRegistry(
    private val factories: List<LlmProviderFactory>,
    private val defaultProviderId: String? = null
) {
    constructor(vararg factories: LlmProviderFactory) : this(factories.toList(), null)

    constructor(
        vararg factories: LlmProviderFactory,
        defaultProviderId: String
    ) : this(factories.toList(), defaultProviderId)

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

    /**
     * Get the default provider factory.
     * Returns the factory matching [defaultProviderId], or the first factory if no default is set.
     */
    fun getDefaultFactory(): LlmProviderFactory? =
        defaultProviderId?.let { getFactory(it) } ?: factories.firstOrNull()

    /**
     * Get the default provider ID.
     */
    fun getDefaultProviderId(): String? =
        defaultProviderId ?: factories.firstOrNull()?.providerId
}
