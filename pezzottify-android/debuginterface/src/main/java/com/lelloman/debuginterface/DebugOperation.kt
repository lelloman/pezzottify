package com.lelloman.debuginterface

sealed class DebugOperation(
    val name: String,
    val description: String?,
) {
    class SimpleAction<T>(name: String, description: String? = null, val action: () -> T) :
        DebugOperation(name, description) {
        internal fun getKey(): String {
            return name.lowercase().replace(" ", "-").replace(Regex("[^a-z\\-]"), "")
        }
    }
}