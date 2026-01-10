package com.lelloman.simpleaiassistant.llm

import kotlinx.coroutines.flow.StateFlow

/**
 * Store for persisting LLM provider configuration.
 *
 * Implementations should persist the selected provider and its configuration
 * (e.g., using SharedPreferences, DataStore, etc.)
 */
interface ProviderConfigStore {

    /**
     * Currently selected provider ID.
     */
    val selectedProviderId: StateFlow<String?>

    /**
     * Configuration for the currently selected provider.
     */
    val config: StateFlow<Map<String, Any?>>

    /**
     * Save provider selection and configuration.
     *
     * @param providerId The provider to select
     * @param config The configuration for that provider
     */
    suspend fun save(providerId: String, config: Map<String, Any?>)

    /**
     * Clear all stored configuration.
     */
    suspend fun clear()
}
