package com.lelloman.pezzottify.android.ui.screen.main.settings.logviewer

// Note: DEBUG level is excluded because FileLogger only writes INFO, WARN, ERROR to file.
// If DEBUG logging is enabled in the future, add DEBUG("Debug") here.
enum class LogLevel(val label: String) {
    INFO("Info"),
    WARN("Warn"),
    ERROR("Error"),
}

data class LogViewerScreenState(
    val logContent: String = "",
    val searchQuery: String = "",
    val isLoading: Boolean = true,
    val isRefreshing: Boolean = false,
    val enabledLevels: Set<LogLevel> = LogLevel.entries.toSet(),
)
