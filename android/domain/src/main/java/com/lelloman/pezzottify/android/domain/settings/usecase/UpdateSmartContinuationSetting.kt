package com.lelloman.pezzottify.android.domain.settings.usecase

import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import javax.inject.Inject

class UpdateSmartContinuationSetting @Inject constructor(
    private val userSettingsStore: UserSettingsStore,
) {
    suspend operator fun invoke(enabled: Boolean) {
        userSettingsStore.setSyncedSetting(
            setting = UserSetting.SmartContinuationEnabled(enabled),
            syncStatus = SyncStatus.PendingSync,
        )
    }
}
