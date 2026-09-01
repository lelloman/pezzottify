package com.lelloman.simpleaiassistant.llm

/** Describes whether an app build lets users change its LLM provider configuration. */
data class ProviderConfigurationPolicy(
    val isUserConfigurable: Boolean
)
