package com.lelloman.pezzottify.android.domain.settings

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.sync.BaseSynchronizer
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

/**
 * Background synchronizer that sends pending user settings to the server.
 * Works similarly to [com.lelloman.pezzottify.android.domain.usercontent.UserContentSynchronizer].
 *
 * Settings are saved locally first (with PendingSync status), then this synchronizer
 * picks them up and syncs them to the server. On network failure, settings remain
 * pending and are retried with exponential backoff.
 */
@Singleton
class UserSettingsSynchronizer internal constructor(
    private val userSettingsStore: UserSettingsStore,
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
) : BaseSynchronizer<SyncedUserSetting>(
    logger = loggerFactory.getLogger(UserSettingsSynchronizer::class),
    dispatcher = dispatcher,
    scope = scope,
    minSleepDuration = MIN_SLEEP_DURATION,
    maxSleepDuration = MAX_SLEEP_DURATION,
) {

    @Inject
    constructor(
        userSettingsStore: UserSettingsStore,
        remoteApiClient: RemoteApiClient,
        loggerFactory: LoggerFactory,
    ) : this(
        userSettingsStore,
        remoteApiClient,
        loggerFactory,
        Dispatchers.IO,
        GlobalScope
    )

    override suspend fun getItemsToProcess(): List<SyncedUserSetting> {
        return userSettingsStore.getPendingSyncSettings().first()
    }

    override suspend fun processItem(item: SyncedUserSetting) {
        syncSetting(item)
    }

    private suspend fun syncSetting(item: SyncedUserSetting) {
        logger.debug("syncSetting() syncing ${item.key}")
        userSettingsStore.updateSyncStatus(item.key, SyncStatus.Syncing)

        val result = remoteApiClient.updateUserSettings(listOf(item.setting))

        when (result) {
            is RemoteApiResponse.Success -> {
                logger.info("syncSetting() success for ${item.key}")
                userSettingsStore.updateSyncStatus(item.key, SyncStatus.Synced)
            }
            is RemoteApiResponse.Error.Network -> {
                logger.debug("syncSetting() network error for ${item.key}, will retry later")
                userSettingsStore.updateSyncStatus(item.key, SyncStatus.PendingSync)
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                logger.warn("syncSetting() unauthorized for ${item.key}")
                userSettingsStore.updateSyncStatus(item.key, SyncStatus.SyncError)
            }
            is RemoteApiResponse.Error.NotFound -> {
                logger.warn("syncSetting() not found for ${item.key}")
                userSettingsStore.updateSyncStatus(item.key, SyncStatus.SyncError)
            }
            is RemoteApiResponse.Error.Unknown -> {
                logger.error("syncSetting() unknown error for ${item.key}: ${result.message}")
                userSettingsStore.updateSyncStatus(item.key, SyncStatus.SyncError)
            }
            is RemoteApiResponse.Error.EventsPruned -> {
                // EventsPruned is not expected for settings operations
                logger.error("syncSetting() unexpected events pruned error for ${item.key}")
                userSettingsStore.updateSyncStatus(item.key, SyncStatus.SyncError)
            }
        }
    }

    private companion object {
        val MIN_SLEEP_DURATION = 100.milliseconds
        val MAX_SLEEP_DURATION = 30.seconds
    }
}
