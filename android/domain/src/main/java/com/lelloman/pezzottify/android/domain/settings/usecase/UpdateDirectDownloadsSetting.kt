package com.lelloman.pezzottify.android.domain.settings.usecase

import com.lelloman.pezzottify.android.domain.settings.UserSettingsSynchronizer
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import javax.inject.Inject

/**
 * Use case for updating the direct downloads setting.
 *
 * The setting is saved locally with PendingSync status, and the synchronizer
 * will pick it up and sync it to the server. This allows the setting to work
 * offline - it will be synced when connectivity is restored.
 */
class UpdateDirectDownloadsSetting @Inject constructor(
    private val userSettingsStore: UserSettingsStore,
    private val userSettingsSynchronizer: UserSettingsSynchronizer,
) {

    suspend operator fun invoke(enabled: Boolean) {
        // Save locally with PendingSync status
        userSettingsStore.setSyncedSetting(
            setting = UserSetting.DirectDownloadsEnabled(enabled),
            syncStatus = SyncStatus.PendingSync,
        )

        // Wake up the synchronizer to process the change
        userSettingsSynchronizer.wakeUp()
    }
}
