package com.lelloman.pezzottify.android.domain.usercontent

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.sync.BaseSynchronizer
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
 * Synchronizes local playlist changes to the server.
 *
 * This synchronizer handles:
 * - Creating new playlists on the server (PendingCreate)
 * - Updating existing playlists on the server (PendingUpdate)
 * - Deleting playlists from the server (PendingDelete)
 */
@Singleton
class PlaylistSynchronizer internal constructor(
    private val userPlaylistStore: UserPlaylistStore,
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
) : BaseSynchronizer<UserPlaylist>(
    logger = loggerFactory.getLogger(PlaylistSynchronizer::class),
    dispatcher = dispatcher,
    scope = scope,
    minSleepDuration = MIN_SLEEP_DURATION,
    maxSleepDuration = MAX_SLEEP_DURATION,
) {

    @Inject
    constructor(
        userPlaylistStore: UserPlaylistStore,
        remoteApiClient: RemoteApiClient,
        loggerFactory: LoggerFactory,
    ) : this(
        userPlaylistStore,
        remoteApiClient,
        loggerFactory,
        Dispatchers.IO,
        GlobalScope
    )

    override suspend fun getItemsToProcess(): List<UserPlaylist> {
        return userPlaylistStore.getPendingSyncPlaylists().first()
    }

    override suspend fun processItem(item: UserPlaylist) {
        when (item.syncStatus) {
            PlaylistSyncStatus.PendingCreate -> syncCreate(item)
            PlaylistSyncStatus.PendingUpdate -> syncUpdate(item)
            PlaylistSyncStatus.PendingDelete -> syncDelete(item)
            else -> {
                // Synced, Syncing, SyncError - shouldn't be in pending list
                logger.warn("processItem() unexpected sync status: ${item.syncStatus} for playlist ${item.id}")
            }
        }
    }

    private suspend fun syncCreate(item: UserPlaylist) {
        logger.debug("syncCreate() creating playlist ${item.id} with name '${item.name}'")
        userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.Syncing)

        when (val result = remoteApiClient.createPlaylist(item.name, item.trackIds)) {
            is RemoteApiResponse.Success -> {
                val serverId = result.data
                logger.info("syncCreate() success for ${item.id}, server assigned id: $serverId")

                // The server assigns its own ID. We need to update our local playlist with the server ID.
                // Delete the old local entry and create a new one with the server ID.
                userPlaylistStore.deletePlaylist(item.id)
                userPlaylistStore.createOrUpdatePlaylist(
                    id = serverId,
                    name = item.name,
                    trackIds = item.trackIds,
                    syncStatus = PlaylistSyncStatus.Synced,
                )
            }
            is RemoteApiResponse.Error.Network -> {
                logger.debug("syncCreate() network error for ${item.id}, will retry later")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.PendingCreate)
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                logger.warn("syncCreate() unauthorized for ${item.id}")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.SyncError)
            }
            else -> {
                logger.error("syncCreate() error for ${item.id}: $result")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.SyncError)
            }
        }
    }

    private suspend fun syncUpdate(item: UserPlaylist) {
        logger.debug("syncUpdate() updating playlist ${item.id}")
        userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.Syncing)

        when (val result = remoteApiClient.updatePlaylist(item.id, item.name, item.trackIds)) {
            is RemoteApiResponse.Success -> {
                logger.info("syncUpdate() success for ${item.id}")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.Synced)
            }
            is RemoteApiResponse.Error.Network -> {
                logger.debug("syncUpdate() network error for ${item.id}, will retry later")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.PendingUpdate)
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                logger.warn("syncUpdate() unauthorized for ${item.id}")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.SyncError)
            }
            is RemoteApiResponse.Error.NotFound -> {
                // Playlist doesn't exist on server, try to create it instead
                logger.warn("syncUpdate() playlist ${item.id} not found on server, will try to create")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.PendingCreate)
            }
            else -> {
                logger.error("syncUpdate() error for ${item.id}: $result")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.SyncError)
            }
        }
    }

    private suspend fun syncDelete(item: UserPlaylist) {
        logger.debug("syncDelete() deleting playlist ${item.id}")
        userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.Syncing)

        when (val result = remoteApiClient.deletePlaylist(item.id)) {
            is RemoteApiResponse.Success -> {
                logger.info("syncDelete() success for ${item.id}")
                // Now we can actually delete it from local storage
                userPlaylistStore.deletePlaylist(item.id)
            }
            is RemoteApiResponse.Error.Network -> {
                logger.debug("syncDelete() network error for ${item.id}, will retry later")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.PendingDelete)
            }
            is RemoteApiResponse.Error.Unauthorized -> {
                logger.warn("syncDelete() unauthorized for ${item.id}")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.SyncError)
            }
            is RemoteApiResponse.Error.NotFound -> {
                // Playlist doesn't exist on server, just delete locally
                logger.info("syncDelete() playlist ${item.id} not found on server, deleting locally")
                userPlaylistStore.deletePlaylist(item.id)
            }
            else -> {
                logger.error("syncDelete() error for ${item.id}: $result")
                userPlaylistStore.updateSyncStatus(item.id, PlaylistSyncStatus.SyncError)
            }
        }
    }

    private companion object {
        val MIN_SLEEP_DURATION = 100.milliseconds
        val MAX_SLEEP_DURATION = 30.seconds
    }
}
