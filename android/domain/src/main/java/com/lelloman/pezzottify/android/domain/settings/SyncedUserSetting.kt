package com.lelloman.pezzottify.android.domain.settings

import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus

/**
 * A user setting that needs to be synchronized with the server.
 * Wraps the actual setting value with sync metadata.
 */
data class SyncedUserSetting(
    val setting: UserSetting,
    val modifiedAt: Long,
    val syncStatus: SyncStatus = SyncStatus.PendingSync,
) {
    /**
     * Unique key for this setting type.
     */
    val key: String
        get() = when (setting) {
            is UserSetting.DirectDownloadsEnabled -> "enable_direct_downloads"
            is UserSetting.ExternalSearchEnabled -> "enable_external_search"
        }
}
