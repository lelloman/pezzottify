package com.lelloman.pezzottify.android.ui.screen.main.profile

data class ProfileScreenState(
    val userName: String = "",
    val baseUrl: String = "",
    val isLoggingOut: Boolean = false,
    val showLogoutConfirmation: Boolean = false,
    val buildVariant: String = "",
    val versionName: String = "",
    val gitCommit: String = "",
    val serverVersion: String = "disconnected",
)
