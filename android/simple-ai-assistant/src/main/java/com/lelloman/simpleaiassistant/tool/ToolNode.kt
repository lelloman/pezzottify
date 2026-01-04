package com.lelloman.simpleaiassistant.tool

/**
 * A node in the tool topography tree.
 */
sealed class ToolNode {
    /**
     * Reference to a tool by ID.
     */
    data class ToolRef(val toolId: String) : ToolNode()

    /**
     * A group of tools/subgroups that can be expanded on-demand.
     */
    data class Group(
        val id: String,
        val name: String,
        val description: String,
        val children: List<ToolNode>
    ) : ToolNode()
}
