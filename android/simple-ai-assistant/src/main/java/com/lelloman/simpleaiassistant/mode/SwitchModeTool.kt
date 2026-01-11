package com.lelloman.simpleaiassistant.mode

import com.lelloman.simpleaiassistant.tool.Tool
import com.lelloman.simpleaiassistant.tool.ToolResult
import com.lelloman.simpleaiassistant.tool.ToolSpec

/**
 * Built-in tool that allows the LLM to switch between assistant modes.
 * This tool is automatically added to every mode's available tools.
 *
 * Actions:
 * - list: Shows all available modes (flat list)
 * - switch: Switches to a specific mode by ID
 * - current: Shows current mode info and breadcrumb path
 *
 * @param modeManager The mode manager to perform mode operations
 * @param onModeSwitch Callback invoked when mode is switched (for history compaction)
 */
class SwitchModeTool(
    private val modeManager: ModeManager,
    private val onModeSwitch: suspend (fromMode: AssistantMode, toMode: AssistantMode) -> Unit = { _, _ -> }
) : Tool {

    override val spec = ToolSpec(
        name = TOOL_NAME,
        description = "Switch between assistant modes. Use 'list' to see all available modes, 'switch' to change to a different mode, or 'current' to see the current mode.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "action" to mapOf(
                    "type" to "string",
                    "enum" to listOf("list", "switch", "current"),
                    "description" to "Action to perform: 'list' shows all modes, 'switch' changes mode, 'current' shows current mode info"
                ),
                "mode_id" to mapOf(
                    "type" to "string",
                    "description" to "The mode ID to switch to (required for 'switch' action)"
                )
            ),
            "required" to listOf("action")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val action = input["action"] as? String
            ?: return ToolResult(success = false, error = "Missing action parameter")

        return when (action) {
            "list" -> executeList()
            "switch" -> executeSwitch(input["mode_id"] as? String)
            "current" -> executeCurrent()
            else -> ToolResult(success = false, error = "Unknown action: $action. Valid actions: list, switch, current")
        }
    }

    private fun executeList(): ToolResult {
        val allModes = modeManager.getAllModes()
        val currentModeId = modeManager.currentMode.value.id

        val formatted = buildString {
            appendLine("Available assistant modes:")
            appendLine()
            allModes.forEach { mode ->
                val isCurrent = if (mode.id == currentModeId) " (current)" else ""
                val hasChildren = if (mode.hasChildren()) " [has sub-modes]" else ""
                appendLine("- ${mode.id}: ${mode.name}$isCurrent$hasChildren")
                appendLine("  ${mode.description}")
            }
        }

        return ToolResult(success = true, data = formatted.trimEnd())
    }

    private suspend fun executeSwitch(targetModeId: String?): ToolResult {
        if (targetModeId == null) {
            return ToolResult(
                success = false,
                error = "Missing mode_id parameter. Use action='list' to see available modes."
            )
        }

        val fromMode = modeManager.currentMode.value

        if (targetModeId == fromMode.id) {
            return ToolResult(
                success = true,
                data = "Already in ${fromMode.name} mode."
            )
        }

        val targetMode = modeManager.getModeTree().findMode(targetModeId)
            ?: return ToolResult(
                success = false,
                error = "Mode not found: $targetModeId. Use action='list' to see available modes."
            )

        // Switch the mode
        modeManager.switchToMode(targetModeId)

        // Notify about the switch (for history compaction)
        onModeSwitch(fromMode, targetMode)

        val toolsInfo = if (targetMode.toolIds.isNotEmpty()) {
            "\n\nAvailable tools in this mode: ${targetMode.toolIds.joinToString(", ")}"
        } else {
            "\n\nThis is a knowledge-only mode with no tools."
        }

        return ToolResult(
            success = true,
            data = "Switched to ${targetMode.name} mode.\n\n${targetMode.description}$toolsInfo"
        )
    }

    private fun executeCurrent(): ToolResult {
        val currentMode = modeManager.currentMode.value
        val path = modeManager.getCurrentPath()
        val pathString = path.joinToString(" > ") { it.name }

        val info = buildString {
            appendLine("Current mode: ${currentMode.name}")
            appendLine("Mode ID: ${currentMode.id}")
            appendLine("Path: $pathString")
            appendLine()
            appendLine("Description: ${currentMode.description}")

            if (currentMode.toolIds.isNotEmpty()) {
                appendLine()
                appendLine("Available tools: ${currentMode.toolIds.joinToString(", ")}")
            }

            if (currentMode.children.isNotEmpty()) {
                appendLine()
                appendLine("Sub-modes:")
                currentMode.children.forEach { child ->
                    appendLine("- ${child.id}: ${child.name}")
                }
            }
        }

        return ToolResult(success = true, data = info.trimEnd())
    }

    companion object {
        const val TOOL_NAME = "switch_mode"
    }
}
