package com.lelloman.simpleaiassistant.util

import com.lelloman.simpleaiassistant.model.Language

/**
 * Interface for persisting language preferences.
 */
interface LanguagePreferences {
    fun getLanguage(): Language?
    fun setLanguage(language: Language?)
}
