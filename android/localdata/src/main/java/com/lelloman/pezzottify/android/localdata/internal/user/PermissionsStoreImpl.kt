package com.lelloman.pezzottify.android.localdata.internal.user

import android.content.Context
import com.lelloman.pezzottify.android.domain.sync.Permission
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * Implementation of [PermissionsStore] using SharedPreferences.
 *
 * Stores user permissions as a JSON array of permission names.
 * Permissions are synced from the server and persisted locally for offline access.
 */
internal class PermissionsStoreImpl(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : PermissionsStore {

    private val prefs = context.getSharedPreferences(SHARED_PREF_FILE_NAME, Context.MODE_PRIVATE)

    private val mutablePermissions by lazy {
        MutableStateFlow(loadPermissions())
    }

    override val permissions: StateFlow<Set<Permission>> = mutablePermissions.asStateFlow()

    override suspend fun setPermissions(permissions: Set<Permission>) {
        withContext(dispatcher) {
            mutablePermissions.value = permissions
            savePermissions(permissions)
        }
    }

    override suspend fun addPermission(permission: Permission) {
        withContext(dispatcher) {
            val updated = mutablePermissions.value + permission
            mutablePermissions.value = updated
            savePermissions(updated)
        }
    }

    override suspend fun removePermission(permission: Permission) {
        withContext(dispatcher) {
            val updated = mutablePermissions.value - permission
            mutablePermissions.value = updated
            savePermissions(updated)
        }
    }

    override suspend fun clear() {
        withContext(dispatcher) {
            mutablePermissions.value = emptySet()
            prefs.edit().remove(KEY_PERMISSIONS).commit()
        }
    }

    private fun loadPermissions(): Set<Permission> {
        val json = prefs.getString(KEY_PERMISSIONS, null) ?: return emptySet()
        return try {
            Json.decodeFromString<List<String>>(json)
                .mapNotNull { name ->
                    try {
                        Permission.valueOf(name)
                    } catch (e: IllegalArgumentException) {
                        null
                    }
                }
                .toSet()
        } catch (e: Exception) {
            emptySet()
        }
    }

    private fun savePermissions(permissions: Set<Permission>) {
        val json = Json.encodeToString(permissions.map { it.name })
        prefs.edit().putString(KEY_PERMISSIONS, json).commit()
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "PermissionsStore"
        private const val KEY_PERMISSIONS = "user_permissions"
    }
}
