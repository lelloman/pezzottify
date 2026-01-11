package com.lelloman.simpleaiassistant.mode

/**
 * Interface for persisting mode selection across app restarts.
 * Apps implement this using SharedPreferences, DataStore, or other storage mechanisms.
 */
interface ModePreferences {
    /**
     * Gets the currently saved mode ID.
     * @return The saved mode ID, or null if none saved (will use root mode)
     */
    fun getCurrentModeId(): String?

    /**
     * Saves the current mode ID.
     * @param modeId The mode ID to save, or null to clear
     */
    fun setCurrentModeId(modeId: String?)
}
