package com.lelloman.simpleaiassistant.llm

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * Immutable provider configuration for builds whose provider is controlled by the distributor.
 */
class FixedProviderConfigStore(
    providerId: String,
    config: Map<String, Any?>
) : ProviderConfigStore {

    override val selectedProviderId: StateFlow<String?> = MutableStateFlow(providerId)
    override val config: StateFlow<Map<String, Any?>> = MutableStateFlow(config.toMap())

    override suspend fun save(providerId: String, config: Map<String, Any?>) = Unit

    override suspend fun clear() = Unit
}
