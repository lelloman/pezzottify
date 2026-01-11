package com.lelloman.simpleaiassistant.util

/**
 * Interface for persisting debug mode preferences.
 */
interface DebugModePreferences {
    fun isDebugMode(): Boolean
    fun setDebugMode(enabled: Boolean)
}
