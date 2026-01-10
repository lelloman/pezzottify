package com.lelloman.simpleaiassistant.llm

/**
 * Describes the configuration schema for an LLM provider.
 * Used to generate settings UI dynamically.
 */
data class ProviderConfigSchema(
    val fields: List<ConfigField>
)

/**
 * A configuration field that can be rendered in a settings UI.
 */
sealed class ConfigField {
    abstract val key: String
    abstract val label: String
    abstract val description: String?

    /**
     * Text input field (e.g., base URL, API key)
     */
    data class Text(
        override val key: String,
        override val label: String,
        override val description: String? = null,
        val default: String = "",
        val placeholder: String = "",
        val isSecret: Boolean = false
    ) : ConfigField()

    /**
     * Numeric input field (e.g., timeout)
     */
    data class Number(
        override val key: String,
        override val label: String,
        override val description: String? = null,
        val default: Long = 0,
        val min: Long? = null,
        val max: Long? = null,
        val suffix: String? = null
    ) : ConfigField()

    /**
     * Dropdown selection field (e.g., model selection)
     * Options can be static or fetched dynamically.
     */
    data class Select(
        override val key: String,
        override val label: String,
        override val description: String? = null,
        val default: String = "",
        val options: List<String> = emptyList(),
        val allowCustom: Boolean = false,
        val dynamicOptions: Boolean = false
    ) : ConfigField()

    /**
     * Boolean toggle field
     */
    data class Toggle(
        override val key: String,
        override val label: String,
        override val description: String? = null,
        val default: Boolean = false
    ) : ConfigField()
}
