package com.lelloman.pezzottify.android.domain.download

import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.Permission
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import javax.inject.Inject

/**
 * Checks if external search feature is enabled for the current user.
 *
 * External search requires both:
 * 1. User has RequestContent permission
 * 2. External search is enabled in user settings
 */
class IsExternalSearchEnabledUseCase @Inject constructor(
    private val permissionsStore: PermissionsStore,
    private val userSettingsStore: UserSettingsStore,
) : UseCase() {

    /**
     * Check if external search is currently enabled.
     *
     * @return true if user has permission and setting is enabled
     */
    suspend operator fun invoke(): Boolean {
        val hasPermission = permissionsStore.permissions.value.contains(Permission.RequestContent)
        val settingEnabled = userSettingsStore.isExternalSearchEnabled.value
        return hasPermission && settingEnabled
    }

    /**
     * Observe external search enabled state.
     * Emits whenever permission or setting changes.
     *
     * @return Flow that emits true when both permission and setting are enabled
     */
    fun observe(): Flow<Boolean> = combine(
        permissionsStore.permissions,
        userSettingsStore.isExternalSearchEnabled,
    ) { permissions, settingEnabled ->
        val hasPermission = permissions.contains(Permission.RequestContent)
        hasPermission && settingEnabled
    }
}
