package com.lelloman.simpleaiassistant.util

import androidx.annotation.StringRes

/**
 * Interface for providing localized strings.
 * This allows the data layer to access string resources without direct Context dependency.
 */
interface StringProvider {
    fun getString(@StringRes resId: Int): String
}
