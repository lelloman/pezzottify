package com.lelloman.simpleaiassistant.data

import com.lelloman.simpleaiassistant.model.Language
import com.lelloman.simpleaiassistant.tool.ToolNode
import com.lelloman.simpleaiassistant.tool.ToolRegistry

/**
 * Default implementation of [SystemPromptBuilder] that provides a generic
 * system prompt suitable for most use cases.
 */
class DefaultSystemPromptBuilder(
    private val assistantName: String = "AI Assistant",
    private val additionalInstructions: String? = null
) : SystemPromptBuilder {

    override fun build(language: Language?, toolRegistry: ToolRegistry): String {
        val parts = mutableListOf<String>()

        // Identity
        parts.add("You are $assistantName, a helpful AI assistant.")

        // Language instruction
        if (language != null) {
            parts.add("Always respond in ${language.displayName} (${language.nativeName}).")
        }

        // Tool instructions
        val rootTools = toolRegistry.getRootSpecs()
        val rootGroups = toolRegistry.getRootGroups()

        if (rootTools.isNotEmpty() || rootGroups.isNotEmpty()) {
            parts.add(buildToolInstructions(toolRegistry, rootGroups))
        }

        // Additional instructions
        if (!additionalInstructions.isNullOrBlank()) {
            parts.add(additionalInstructions)
        }

        return parts.joinToString("\n\n")
    }

    private fun buildToolInstructions(
        toolRegistry: ToolRegistry,
        rootGroups: List<ToolNode.Group>
    ): String {
        val sb = StringBuilder()
        sb.appendLine("You have access to tools that help you accomplish tasks.")

        if (rootGroups.isNotEmpty()) {
            sb.appendLine()
            sb.appendLine("Tool groups available for expansion:")
            for (group in rootGroups) {
                sb.appendLine("- ${group.name}: ${group.description}")
                appendGroupChildren(sb, toolRegistry, group, indent = "  ")
            }
        }

        sb.appendLine()
        sb.appendLine("When using tools:")
        sb.appendLine("- Only use tools when necessary to fulfill the user's request")
        sb.appendLine("- Explain what you're doing when using tools")
        sb.appendLine("- If a tool fails, explain the error and suggest alternatives")

        return sb.toString().trim()
    }

    private fun appendGroupChildren(
        sb: StringBuilder,
        toolRegistry: ToolRegistry,
        group: ToolNode.Group,
        indent: String
    ) {
        for (child in group.children) {
            when (child) {
                is ToolNode.ToolRef -> {
                    val tool = toolRegistry.findById(child.toolId)
                    if (tool != null) {
                        sb.appendLine("$indent- ${tool.spec.name}: ${tool.spec.description}")
                    }
                }
                is ToolNode.Group -> {
                    sb.appendLine("$indent- ${child.name}: ${child.description}")
                    appendGroupChildren(sb, toolRegistry, child, "$indent  ")
                }
            }
        }
    }
}
