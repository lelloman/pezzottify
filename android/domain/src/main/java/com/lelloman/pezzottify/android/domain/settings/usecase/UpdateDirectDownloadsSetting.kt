package com.lelloman.pezzottify.android.domain.settings.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import javax.inject.Inject

/**
 * Use case for updating the direct downloads setting.
 *
 * This performs an optimistic update - the local setting is updated immediately,
 * and if the server request fails, the change is rolled back.
 */
class UpdateDirectDownloadsSetting @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val userSettingsStore: UserSettingsStore,
) {

    suspend operator fun invoke(enabled: Boolean): Result {
        val previousValue = userSettingsStore.directDownloadsEnabled.value

        // Optimistically update local store
        userSettingsStore.setDirectDownloadsEnabled(enabled)

        // Sync with server (this generates a sync event for other devices)
        val response = remoteApiClient.updateUserSettings(
            listOf(UserSetting.DirectDownloadsEnabled(enabled))
        )

        return when (response) {
            is RemoteApiResponse.Success -> Result.Success
            is RemoteApiResponse.Error -> {
                // Revert on failure
                userSettingsStore.setDirectDownloadsEnabled(previousValue)
                Result.Error
            }
        }
    }

    sealed interface Result {
        data object Success : Result
        data object Error : Result
    }
}
