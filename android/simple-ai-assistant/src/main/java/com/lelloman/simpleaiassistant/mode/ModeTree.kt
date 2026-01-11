package com.lelloman.simpleaiassistant.mode

/**
 * Represents the complete mode hierarchy for an assistant.
 * Provides utilities for finding modes, getting paths, and validation.
 *
 * @param root The root mode - the starting point for the assistant
 */
class ModeTree(val root: AssistantMode) {

    /**
     * Flat map of all modes by ID for quick lookup.
     * Built lazily on first access.
     */
    private val modeMap: Map<String, AssistantMode> by lazy {
        buildModeMap(root)
    }

    /**
     * Finds a mode by ID anywhere in the tree.
     * @return The mode with the given ID, or null if not found
     */
    fun findMode(id: String): AssistantMode? = modeMap[id]

    /**
     * Returns all modes in the tree as a flat list.
     */
    fun allModes(): List<AssistantMode> = modeMap.values.toList()

    /**
     * Returns all mode IDs in the tree.
     */
    fun allModeIds(): Set<String> = modeMap.keys

    /**
     * Gets the path from root to a specific mode (breadcrumb).
     * @return List of modes from root to the target, or empty if not found
     */
    fun getPath(modeId: String): List<AssistantMode> {
        return findPath(root, modeId) ?: emptyList()
    }

    /**
     * Gets the parent mode of a given mode.
     * @return The parent mode, or null if the mode is root or not found
     */
    fun getParent(modeId: String): AssistantMode? {
        if (modeId == root.id) return null
        return findParent(root, modeId)
    }

    /**
     * Validates that all tool IDs referenced by modes exist in the given set.
     * @param availableToolIds Set of tool IDs that exist in the ToolRegistry
     * @return List of validation error messages (empty if valid)
     */
    fun validate(availableToolIds: Set<String>): List<String> {
        val errors = mutableListOf<String>()
        modeMap.values.forEach { mode ->
            val missingTools = mode.toolIds - availableToolIds
            if (missingTools.isNotEmpty()) {
                errors.add("Mode '${mode.id}' references unknown tools: $missingTools")
            }
        }
        // Check for duplicate mode IDs (shouldn't happen with proper construction)
        val allIds = mutableSetOf<String>()
        val duplicates = mutableSetOf<String>()
        collectDuplicateIds(root, allIds, duplicates)
        if (duplicates.isNotEmpty()) {
            errors.add("Duplicate mode IDs found: $duplicates")
        }
        return errors
    }

    /**
     * Returns the total number of modes in the tree.
     */
    fun size(): Int = modeMap.size

    private fun buildModeMap(mode: AssistantMode): Map<String, AssistantMode> {
        val result = mutableMapOf<String, AssistantMode>()
        result[mode.id] = mode
        mode.children.forEach { child ->
            result.putAll(buildModeMap(child))
        }
        return result
    }

    private fun findPath(current: AssistantMode, targetId: String): List<AssistantMode>? {
        if (current.id == targetId) return listOf(current)
        for (child in current.children) {
            val path = findPath(child, targetId)
            if (path != null) return listOf(current) + path
        }
        return null
    }

    private fun findParent(current: AssistantMode, targetId: String): AssistantMode? {
        for (child in current.children) {
            if (child.id == targetId) return current
            val parent = findParent(child, targetId)
            if (parent != null) return parent
        }
        return null
    }

    private fun collectDuplicateIds(
        mode: AssistantMode,
        seen: MutableSet<String>,
        duplicates: MutableSet<String>
    ) {
        if (mode.id in seen) {
            duplicates.add(mode.id)
        } else {
            seen.add(mode.id)
        }
        mode.children.forEach { collectDuplicateIds(it, seen, duplicates) }
    }
}
