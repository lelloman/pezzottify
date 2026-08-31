package com.lelloman.pezzottify.android.domain.settings.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import javax.inject.Inject

/**
 * Updates proxy mode on the server before exposing the new value locally.
 *
 * Unlike settings that only affect this client, proxy mode is evaluated by the
 * server when an audio request starts. Keeping the local value unchanged until
 * the server accepts it prevents the UI from offering tracks that the server
 * would still reject as unavailable.
 */
class UpdateProxyModeSetting @Inject constructor(
    private val userSettingsStore: UserSettingsStore,
    private val remoteApiClient: RemoteApiClient,
) {
    suspend operator fun invoke(enabled: Boolean): Boolean {
        val setting = UserSetting.ProxyModeEnabled(enabled)
        return when (remoteApiClient.updateUserSettings(listOf(setting))) {
            is RemoteApiResponse.Success -> {
                userSettingsStore.setSyncedSetting(setting, SyncStatus.Synced)
                true
            }

            is RemoteApiResponse.Error -> false
        }
    }
}
