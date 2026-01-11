package com.lelloman.simpleaiassistant.mode

/**
 * Represents an assistant mode with its own tools, prompt instructions, and optional sub-modes.
 * Modes form a hierarchical tree where each mode has access to specific tools and can have children.
 *
 * @param id Unique identifier for this mode (used for navigation and persistence)
 * @param name Display name shown in UI
 * @param description Description for the LLM to understand when to use this mode
 * @param toolIds Set of tool IDs available in this mode (looked up from central ToolRegistry)
 * @param promptInstructions Additional system prompt instructions specific to this mode
 * @param children Sub-modes accessible from this mode
 */
data class AssistantMode(
    val id: String,
    val name: String,
    val description: String,
    val toolIds: Set<String> = emptySet(),
    val promptInstructions: String = "",
    val children: List<AssistantMode> = emptyList()
) {
    /**
     * Recursively collects all mode IDs in this subtree (including this mode).
     */
    fun allModeIds(): Set<String> {
        return setOf(id) + children.flatMap { it.allModeIds() }
    }

    /**
     * Recursively finds a mode by ID in this subtree.
     * @return The mode with the given ID, or null if not found
     */
    fun findById(targetId: String): AssistantMode? {
        if (id == targetId) return this
        return children.firstNotNullOfOrNull { it.findById(targetId) }
    }

    /**
     * Returns true if this mode has any children.
     */
    fun hasChildren(): Boolean = children.isNotEmpty()

    /**
     * Returns the total number of modes in this subtree (including this mode).
     */
    fun totalModeCount(): Int = 1 + children.sumOf { it.totalModeCount() }
}
