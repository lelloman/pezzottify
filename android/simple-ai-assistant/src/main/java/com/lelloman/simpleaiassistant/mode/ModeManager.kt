package com.lelloman.simpleaiassistant.mode

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Manages the current mode state and provides mode navigation.
 * Handles mode switching, persistence, and exposes reactive state.
 *
 * @param modeTree The complete mode hierarchy
 * @param modePreferences Interface for persisting mode selection
 */
class ModeManager(
    private val modeTree: ModeTree,
    private val modePreferences: ModePreferences
) {
    private val _currentMode = MutableStateFlow(loadInitialMode())

    /**
     * The currently active mode as a reactive StateFlow.
     */
    val currentMode: StateFlow<AssistantMode> = _currentMode.asStateFlow()

    /**
     * Returns all modes in the tree as a flat list.
     * Used for displaying all available modes in UI.
     */
    fun getAllModes(): List<AssistantMode> = modeTree.allModes()

    /**
     * Returns the root mode of the tree.
     */
    fun getRootMode(): AssistantMode = modeTree.root

    /**
     * Switches to a mode by ID.
     * @param modeId The ID of the mode to switch to
     * @return true if switch was successful, false if mode not found
     */
    fun switchToMode(modeId: String): Boolean {
        val targetMode = modeTree.findMode(modeId) ?: return false
        _currentMode.value = targetMode
        modePreferences.setCurrentModeId(modeId)
        return true
    }

    /**
     * Switches to the root mode.
     */
    fun switchToRoot() {
        _currentMode.value = modeTree.root
        modePreferences.setCurrentModeId(modeTree.root.id)
    }

    /**
     * Gets the path from root to the current mode (breadcrumb).
     */
    fun getCurrentPath(): List<AssistantMode> {
        return modeTree.getPath(_currentMode.value.id)
    }

    /**
     * Gets the parent of the current mode.
     * @return The parent mode, or null if current mode is root
     */
    fun getCurrentParent(): AssistantMode? {
        return modeTree.getParent(_currentMode.value.id)
    }

    /**
     * Gets the tool IDs available in the current mode.
     */
    fun getCurrentToolIds(): Set<String> {
        return _currentMode.value.toolIds
    }

    /**
     * Gets the children (sub-modes) of the current mode.
     */
    fun getCurrentChildren(): List<AssistantMode> {
        return _currentMode.value.children
    }

    /**
     * Returns the ModeTree for validation or inspection.
     */
    fun getModeTree(): ModeTree = modeTree

    private fun loadInitialMode(): AssistantMode {
        val savedId = modePreferences.getCurrentModeId()
        return savedId?.let { modeTree.findMode(it) } ?: modeTree.root
    }
}
