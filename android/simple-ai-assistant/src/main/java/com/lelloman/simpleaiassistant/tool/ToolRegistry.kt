package com.lelloman.simpleaiassistant.tool

/**
 * Registry of all available tools.
 * The host app provides tools via this registry.
 *
 * Tools are stored flat by ID, and a tree topography defines their organization:
 * - Root-level ToolRefs: Full definitions sent with every prompt
 * - Groups: Summaries shown, LLM can expand to see contents when needed
 */
class ToolRegistry(
    private val tools: Map<String, Tool>,
    private val topography: List<ToolNode>
) {
    /**
     * Get the root-level topography.
     */
    fun getTopography(): List<ToolNode> = topography

    /**
     * Get root-level tools (ToolRefs at the top level of topography).
     */
    fun getRootTools(): List<Tool> = topography
        .filterIsInstance<ToolNode.ToolRef>()
        .mapNotNull { tools[it.toolId] }

    /**
     * Get specs for root-level tools.
     */
    fun getRootSpecs(): List<ToolSpec> = getRootTools().map { it.spec }

    /**
     * Get root-level groups.
     */
    fun getRootGroups(): List<ToolNode.Group> =
        topography.filterIsInstance<ToolNode.Group>()

    /**
     * Find a group by ID (searches entire tree).
     */
    fun findGroup(groupId: String): ToolNode.Group? = findGroupInNodes(groupId, topography)

    private fun findGroupInNodes(groupId: String, nodes: List<ToolNode>): ToolNode.Group? {
        for (node in nodes) {
            when (node) {
                is ToolNode.ToolRef -> continue
                is ToolNode.Group -> {
                    if (node.id == groupId) return node
                    findGroupInNodes(groupId, node.children)?.let { return it }
                }
            }
        }
        return null
    }

    /**
     * Get tools within a group (direct children only).
     */
    fun getToolsInGroup(group: ToolNode.Group): List<Tool> = group.children
        .filterIsInstance<ToolNode.ToolRef>()
        .mapNotNull { tools[it.toolId] }

    /**
     * Find a tool by ID.
     */
    fun findById(toolId: String): Tool? = tools[toolId]

    /**
     * Get ALL tools (both root-level and those inside groups).
     */
    fun getAllTools(): List<Tool> = tools.values.toList()

    /**
     * Get specs for ALL tools.
     */
    fun getAllSpecs(): List<ToolSpec> = getAllTools().map { it.spec }
}
