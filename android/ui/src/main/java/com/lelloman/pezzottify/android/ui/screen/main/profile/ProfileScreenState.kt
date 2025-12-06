package com.lelloman.pezzottify.android.ui.screen.main.profile

import com.lelloman.pezzottify.android.ui.model.Permission

data class ProfileScreenState(
    val userName: String = "",
    val baseUrl: String = "",
    val isLoggingOut: Boolean = false,
    val showLogoutConfirmation: Boolean = false,
    val buildVariant: String = "",
    val versionName: String = "",
    val gitCommit: String = "",
    val serverVersion: String = "disconnected",
    val permissions: Set<Permission> = emptySet(),
    val selectedPermission: Permission? = null,
)
