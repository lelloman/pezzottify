package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.domain.sync.Permission
import kotlinx.coroutines.flow.StateFlow

/**
 * Store for managing user permissions.
 * Permissions are received from the server via sync and stored locally.
 */
interface PermissionsStore {
    /**
     * Observable set of current user permissions.
     */
    val permissions: StateFlow<Set<Permission>>

    /**
     * Replace all permissions with the given set.
     * Used during full sync.
     */
    suspend fun setPermissions(permissions: Set<Permission>)

    /**
     * Add a single permission.
     * Used when permission_granted event is received.
     */
    suspend fun addPermission(permission: Permission)

    /**
     * Remove a single permission.
     * Used when permission_revoked event is received.
     */
    suspend fun removePermission(permission: Permission)

    /**
     * Clear all permissions.
     * Called on logout.
     */
    suspend fun clear()
}
