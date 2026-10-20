package com.lelloman.pezzottify.android.ui.screen.maintenance

data class MaintenanceScreenState(
    val isLoading: Boolean = true,
    val isLoggingOut: Boolean = false,
    val isLoggedIn: Boolean = false,
    val message: String? = null,
    val estimatedEnd: String? = null,
    val error: Boolean = false,
)

sealed interface MaintenanceScreenEvent {
    data class MaintenanceEnded(val isLoggedIn: Boolean) : MaintenanceScreenEvent
}
