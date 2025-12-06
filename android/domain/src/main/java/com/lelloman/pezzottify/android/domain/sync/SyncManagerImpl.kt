package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class SyncManagerImpl internal constructor(
    private val remoteApiClient: RemoteApiClient,
    private val syncStateStore: SyncStateStore,
    private val userContentStore: UserContentStore,
    private val permissionsStore: PermissionsStore,
    private val logger: Logger,
    private val dispatcher: CoroutineDispatcher,
) : SyncManager {

    @Inject
    constructor(
        remoteApiClient: RemoteApiClient,
        syncStateStore: SyncStateStore,
        userContentStore: UserContentStore,
        permissionsStore: PermissionsStore,
        loggerFactory: LoggerFactory,
    ) : this(
        remoteApiClient,
        syncStateStore,
        userContentStore,
        permissionsStore,
        loggerFactory.getLogger(SyncManagerImpl::class),
        Dispatchers.IO,
    )

    private val mutableState = MutableStateFlow<SyncState>(SyncState.Idle)
    override val state: StateFlow<SyncState> = mutableState.asStateFlow()

    override suspend fun initialize(): Boolean = withContext(dispatcher) {
        logger.info("initialize()")

        val cursor = syncStateStore.getCursor()

        if (cursor == 0L) {
            // No cursor, do full sync
            logger.info("No cursor, performing full sync")
            fullSync()
        } else {
            // Have cursor, catch up
            logger.info("Cursor at $cursor, catching up")
            catchUp()
        }
    }

    override suspend fun fullSync(): Boolean = withContext(dispatcher) {
        logger.info("fullSync()")
        mutableState.value = SyncState.Syncing

        when (val response = remoteApiClient.getSyncState()) {
            is RemoteApiResponse.Success -> {
                val syncState = response.data
                logger.info("Full sync received, seq=${syncState.seq}")

                // Update liked content
                applyLikesState(syncState.likes.albums, LikedContent.ContentType.Album)
                applyLikesState(syncState.likes.artists, LikedContent.ContentType.Artist)
                applyLikesState(syncState.likes.tracks, LikedContent.ContentType.Track)

                // Update permissions
                permissionsStore.setPermissions(syncState.permissions.toSet())
                logger.debug("Applied permissions: ${syncState.permissions}")

                // TODO: Apply settings, playlists when those stores exist

                // Save cursor
                syncStateStore.saveCursor(syncState.seq)
                mutableState.value = SyncState.Synced(syncState.seq)
                logger.info("Full sync complete, cursor=${syncState.seq}")
                true
            }

            is RemoteApiResponse.Error -> {
                val errorMsg = errorToMessage(response)
                logger.error("Full sync failed: $errorMsg")
                mutableState.value = SyncState.Error(errorMsg)
                false
            }
        }
    }

    override suspend fun catchUp(): Boolean = withContext(dispatcher) {
        logger.info("catchUp()")
        mutableState.value = SyncState.Syncing

        val cursor = syncStateStore.getCursor()
        when (val response = remoteApiClient.getSyncEvents(cursor)) {
            is RemoteApiResponse.Success -> {
                val eventsResponse = response.data

                // Check for sequence gap
                if (eventsResponse.events.isNotEmpty() &&
                    eventsResponse.events.first().seq > cursor + 1
                ) {
                    logger.info("Sequence gap detected, performing full sync")
                    return@withContext fullSync()
                }

                // Apply events in order
                for (storedEvent in eventsResponse.events) {
                    applyStoredEvent(storedEvent)
                    syncStateStore.saveCursor(storedEvent.seq)
                }

                // Update cursor to current even if no events
                if (eventsResponse.currentSeq > cursor) {
                    syncStateStore.saveCursor(eventsResponse.currentSeq)
                }

                mutableState.value = SyncState.Synced(eventsResponse.currentSeq)
                logger.info("Catch-up complete, cursor=${eventsResponse.currentSeq}")
                true
            }

            is RemoteApiResponse.Error.EventsPruned -> {
                logger.info("Events pruned, performing full sync")
                fullSync()
            }

            is RemoteApiResponse.Error -> {
                val errorMsg = errorToMessage(response)
                logger.error("Catch-up failed: $errorMsg")
                mutableState.value = SyncState.Error(errorMsg)
                false
            }
        }
    }

    override suspend fun handleSyncMessage(storedEvent: StoredEvent) = withContext(dispatcher) {
        logger.debug("handleSyncMessage() seq=${storedEvent.seq}")

        val cursor = syncStateStore.getCursor()

        // Check for sequence gap
        if (storedEvent.seq > cursor + 1) {
            logger.info("WebSocket sequence gap detected, catching up")
            catchUp()
            return@withContext
        }

        // Apply the event
        applyStoredEvent(storedEvent)
        syncStateStore.saveCursor(storedEvent.seq)
        mutableState.value = SyncState.Synced(storedEvent.seq)
    }

    override suspend fun cleanup() = withContext(dispatcher) {
        logger.info("cleanup()")
        syncStateStore.clearCursor()
        mutableState.value = SyncState.Idle
    }

    private suspend fun applyStoredEvent(storedEvent: StoredEvent) {
        val event = storedEvent.toSyncEvent()
        if (event == null) {
            logger.warn("Unknown event type: ${storedEvent.type}")
            return
        }

        when (event) {
            is SyncEvent.ContentLiked -> {
                val contentType = likedContentTypeFrom(event.contentType)
                if (contentType != null) {
                    userContentStore.setLiked(
                        contentId = event.contentId,
                        type = contentType,
                        liked = true,
                        modifiedAt = storedEvent.serverTimestamp,
                        syncStatus = SyncStatus.Synced,
                    )
                    logger.debug("Applied ContentLiked: ${event.contentType} ${event.contentId}")
                }
            }

            is SyncEvent.ContentUnliked -> {
                val contentType = likedContentTypeFrom(event.contentType)
                if (contentType != null) {
                    userContentStore.setLiked(
                        contentId = event.contentId,
                        type = contentType,
                        liked = false,
                        modifiedAt = storedEvent.serverTimestamp,
                        syncStatus = SyncStatus.Synced,
                    )
                    logger.debug("Applied ContentUnliked: ${event.contentType} ${event.contentId}")
                }
            }

            is SyncEvent.SettingChanged -> {
                // TODO: Implement when settings store is available
                logger.debug("Applied SettingChanged: ${event.setting}")
            }

            is SyncEvent.PlaylistCreated -> {
                // TODO: Implement when playlist store is available
                logger.debug("Applied PlaylistCreated: ${event.playlistId} ${event.name}")
            }

            is SyncEvent.PlaylistRenamed -> {
                // TODO: Implement when playlist store is available
                logger.debug("Applied PlaylistRenamed: ${event.playlistId} ${event.name}")
            }

            is SyncEvent.PlaylistDeleted -> {
                // TODO: Implement when playlist store is available
                logger.debug("Applied PlaylistDeleted: ${event.playlistId}")
            }

            is SyncEvent.PlaylistTracksUpdated -> {
                // TODO: Implement when playlist store is available
                logger.debug("Applied PlaylistTracksUpdated: ${event.playlistId}")
            }

            is SyncEvent.PermissionGranted -> {
                permissionsStore.addPermission(event.permission)
                logger.debug("Applied PermissionGranted: ${event.permission}")
            }

            is SyncEvent.PermissionRevoked -> {
                permissionsStore.removePermission(event.permission)
                logger.debug("Applied PermissionRevoked: ${event.permission}")
            }

            is SyncEvent.PermissionsReset -> {
                permissionsStore.setPermissions(event.permissions.toSet())
                logger.debug("Applied PermissionsReset: ${event.permissions}")
            }
        }
    }

    private suspend fun applyLikesState(contentIds: List<String>, type: LikedContent.ContentType) {
        val now = System.currentTimeMillis()
        val items = contentIds.map { contentId ->
            object : LikedContent {
                override val contentId = contentId
                override val contentType = type
                override val isLiked = true
                override val modifiedAt = now
                override val syncStatus = SyncStatus.Synced
            }
        }

        // For full sync, we replace all content of this type
        // This is a simplified approach - a more complete implementation
        // would merge with existing local data
        items.forEach { item ->
            userContentStore.setLiked(
                contentId = item.contentId,
                type = item.contentType,
                liked = item.isLiked,
                modifiedAt = item.modifiedAt,
                syncStatus = item.syncStatus,
            )
        }
    }

    private fun likedContentTypeFrom(type: LikedContentType): LikedContent.ContentType? {
        return when (type) {
            LikedContentType.Album -> LikedContent.ContentType.Album
            LikedContentType.Artist -> LikedContent.ContentType.Artist
            LikedContentType.Track -> LikedContent.ContentType.Track
        }
    }

    private fun errorToMessage(error: RemoteApiResponse.Error): String {
        return when (error) {
            is RemoteApiResponse.Error.Network -> "Network error"
            is RemoteApiResponse.Error.Unauthorized -> "Unauthorized"
            is RemoteApiResponse.Error.NotFound -> "Not found"
            is RemoteApiResponse.Error.EventsPruned -> "Events pruned"
            is RemoteApiResponse.Error.Unknown -> error.message
        }
    }
}
