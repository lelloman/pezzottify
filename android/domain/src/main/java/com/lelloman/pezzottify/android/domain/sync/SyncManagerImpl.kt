package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository
import com.lelloman.pezzottify.android.domain.notifications.DownloadCompletedData
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepository
import com.lelloman.pezzottify.android.domain.notifications.NotificationType
import com.lelloman.pezzottify.android.domain.notifications.SystemNotificationHelper
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.PlaylistState
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.time.Duration
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds

@Singleton
class SyncManagerImpl internal constructor(
    private val remoteApiClient: RemoteApiClient,
    private val syncStateStore: SyncStateStore,
    private val userContentStore: UserContentStore,
    private val userPlaylistStore: UserPlaylistStore,
    private val permissionsStore: PermissionsStore,
    private val userSettingsStore: UserSettingsStore,
    private val downloadStatusRepository: DownloadStatusRepository,
    private val notificationRepository: NotificationRepository,
    private val systemNotificationHelper: SystemNotificationHelper,
    private val logger: Logger,
    private val dispatcher: CoroutineDispatcher,
    private val scope: CoroutineScope,
    private val minRetryDelay: Duration = MIN_RETRY_DELAY,
    private val maxRetryDelay: Duration = MAX_RETRY_DELAY,
) : SyncManager {

    @Inject
    constructor(
        remoteApiClient: RemoteApiClient,
        syncStateStore: SyncStateStore,
        userContentStore: UserContentStore,
        userPlaylistStore: UserPlaylistStore,
        permissionsStore: PermissionsStore,
        userSettingsStore: UserSettingsStore,
        downloadStatusRepository: DownloadStatusRepository,
        notificationRepository: NotificationRepository,
        systemNotificationHelper: SystemNotificationHelper,
        loggerFactory: LoggerFactory,
    ) : this(
        remoteApiClient,
        syncStateStore,
        userContentStore,
        userPlaylistStore,
        permissionsStore,
        userSettingsStore,
        downloadStatusRepository,
        notificationRepository,
        systemNotificationHelper,
        loggerFactory.getLogger(SyncManagerImpl::class),
        Dispatchers.IO,
        CoroutineScope(SupervisorJob() + Dispatchers.IO),
    )

    private val mutableState = MutableStateFlow<SyncState>(SyncState.Idle)
    override val state: StateFlow<SyncState> = mutableState.asStateFlow()

    private var retryJob: Job? = null
    private var currentRetryDelay: Duration = minRetryDelay

    // Debounce download completion notifications so multiple arrivals get grouped
    // Each entry pairs the internal notification ID with the download data
    private val pendingDownloadNotifications = mutableListOf<Pair<String, DownloadCompletedData>>()
    private var downloadNotificationFlushJob: Job? = null

    override suspend fun initialize(): Boolean = withContext(dispatcher) {
        logger.info("initialize()")

        val cursor = syncStateStore.getCurrentCursor()
        val needsFullSync = syncStateStore.needsFullSync()

        if (cursor == 0L || needsFullSync) {
            // No cursor or full sync needed, do full sync
            logger.info("Performing full sync (cursor=$cursor, needsFullSync=$needsFullSync)")
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

                // Apply settings
                applySettingsState(syncState.settings)

                // Apply playlists
                applyPlaylistsState(syncState.playlists)

                // Apply notifications
                notificationRepository.setNotifications(syncState.notifications)
                logger.debug("Applied ${syncState.notifications.size} notifications")

                // Show system notifications for unread download completions
                showSystemNotificationsForUnread(syncState.notifications)

                // Save cursor and clear needsFullSync flag
                syncStateStore.saveCursor(syncState.seq)
                syncStateStore.setNeedsFullSync(false)
                mutableState.value = SyncState.Synced(syncState.seq)
                // Reset retry delay on success
                currentRetryDelay = minRetryDelay
                logger.info("Full sync complete, cursor=${syncState.seq}")

                // Process any pending notification reads that were queued while offline
                processPendingNotificationReads()

                true
            }

            is RemoteApiResponse.Error -> {
                val errorMsg = errorToMessage(response)
                logger.error("Full sync failed: $errorMsg")
                // Mark that we need full sync (persisted across app restarts)
                syncStateStore.setNeedsFullSync(true)
                mutableState.value = SyncState.Error(errorMsg)
                scheduleRetry()
                false
            }
        }
    }

    override suspend fun catchUp(): Boolean = withContext(dispatcher) {
        logger.info("catchUp()")
        mutableState.value = SyncState.Syncing

        val cursor = syncStateStore.getCurrentCursor()
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

                // Clear needsFullSync flag on successful catch-up
                syncStateStore.setNeedsFullSync(false)
                mutableState.value = SyncState.Synced(eventsResponse.currentSeq)
                // Reset retry delay on success
                currentRetryDelay = minRetryDelay
                logger.info("Catch-up complete, cursor=${eventsResponse.currentSeq}")

                // Process any pending notification reads that were queued while offline
                processPendingNotificationReads()

                true
            }

            is RemoteApiResponse.Error.EventsPruned -> {
                logger.info("Events pruned, marking full sync needed")
                // Mark that we need full sync before attempting it
                syncStateStore.setNeedsFullSync(true)
                fullSync()
            }

            is RemoteApiResponse.Error -> {
                val errorMsg = errorToMessage(response)
                logger.error("Catch-up failed: $errorMsg")
                mutableState.value = SyncState.Error(errorMsg)
                scheduleRetry()
                false
            }
        }
    }

    override suspend fun handleSyncMessage(storedEvent: StoredEvent) = withContext(dispatcher) {
        logger.debug("handleSyncMessage() seq=${storedEvent.seq}")

        val cursor = syncStateStore.getCurrentCursor()

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
        cancelRetry()
        downloadNotificationFlushJob?.cancel()
        synchronized(pendingDownloadNotifications) { pendingDownloadNotifications.clear() }
        syncStateStore.clearCursor()
        downloadStatusRepository.clear()
        notificationRepository.clear()
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
                applySetting(event.setting)
                logger.debug("Applied SettingChanged: ${event.setting}")
            }

            is SyncEvent.PlaylistCreated -> {
                userPlaylistStore.createOrUpdatePlaylist(
                    id = event.playlistId,
                    name = event.name,
                    trackIds = emptyList(),
                )
                logger.debug("Applied PlaylistCreated: ${event.playlistId} ${event.name}")
            }

            is SyncEvent.PlaylistRenamed -> {
                userPlaylistStore.updatePlaylistName(event.playlistId, event.name, fromServer = true)
                logger.debug("Applied PlaylistRenamed: ${event.playlistId} ${event.name}")
            }

            is SyncEvent.PlaylistDeleted -> {
                userPlaylistStore.deletePlaylist(event.playlistId)
                logger.debug("Applied PlaylistDeleted: ${event.playlistId}")
            }

            is SyncEvent.PlaylistTracksUpdated -> {
                userPlaylistStore.updatePlaylistTracks(event.playlistId, event.trackIds, fromServer = true)
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

            // Download status events - forward to repository for UI updates
            is SyncEvent.DownloadRequestCreated -> {
                downloadStatusRepository.onRequestCreated(
                    requestId = event.requestId,
                    contentId = event.contentId,
                    contentName = event.contentName,
                    artistName = event.artistName,
                    queuePosition = event.queuePosition,
                )
                logger.debug("Applied DownloadRequestCreated: ${event.requestId} for ${event.contentName}")
            }

            is SyncEvent.DownloadStatusChanged -> {
                downloadStatusRepository.onStatusChanged(
                    requestId = event.requestId,
                    contentId = event.contentId,
                    status = event.status,
                    queuePosition = event.queuePosition,
                    errorMessage = event.errorMessage,
                )
                logger.debug("Applied DownloadStatusChanged: ${event.requestId} -> ${event.status}")
            }

            is SyncEvent.DownloadProgressUpdated -> {
                downloadStatusRepository.onProgressUpdated(
                    requestId = event.requestId,
                    contentId = event.contentId,
                    progress = event.progress,
                )
                logger.debug("Applied DownloadProgressUpdated: ${event.requestId} ${event.progress.completed}/${event.progress.totalChildren}")
            }

            is SyncEvent.DownloadCompleted -> {
                downloadStatusRepository.onCompleted(
                    requestId = event.requestId,
                    contentId = event.contentId,
                )
                logger.debug("Applied DownloadCompleted: ${event.requestId} content=${event.contentId}")
            }

            is SyncEvent.NotificationCreated -> {
                notificationRepository.onNotificationCreated(event.notification)
                // Queue system notification for download completed (debounced to group multiple)
                if (event.notification.notificationType == NotificationType.DownloadCompleted) {
                    try {
                        val data = kotlinx.serialization.json.Json.decodeFromJsonElement(
                            DownloadCompletedData.serializer(),
                            event.notification.data
                        )
                        scheduleDownloadNotification(event.notification.id, data)
                    } catch (e: Exception) {
                        logger.error("Failed to parse download completed notification data", e)
                    }
                }
                logger.debug("Applied NotificationCreated: ${event.notification.id} ${event.notification.title}")
            }

            is SyncEvent.NotificationRead -> {
                notificationRepository.onNotificationRead(event.notificationId, event.readAt)
                logger.debug("Applied NotificationRead: ${event.notificationId}")
            }

            is SyncEvent.WhatsNewBatchClosed -> {
                // Check if user has opted in to WhatsNew notifications
                if (userSettingsStore.isNotifyWhatsNewEnabled.value) {
                    systemNotificationHelper.showWhatsNewNotification(
                        batchId = event.batchId,
                        batchName = event.batchName,
                        description = event.description,
                        albumsAdded = event.albumsAdded,
                        artistsAdded = event.artistsAdded,
                        tracksAdded = event.tracksAdded,
                    )
                    logger.debug("Showed WhatsNew notification: ${event.batchId} ${event.batchName}")
                } else {
                    logger.debug("Skipped WhatsNew notification (user opted out): ${event.batchId}")
                }
            }
        }
    }

    private fun scheduleDownloadNotification(notificationId: String, data: DownloadCompletedData) {
        synchronized(pendingDownloadNotifications) {
            pendingDownloadNotifications.add(notificationId to data)
        }
        downloadNotificationFlushJob?.cancel()
        downloadNotificationFlushJob = scope.launch {
            delay(DOWNLOAD_NOTIFICATION_DEBOUNCE_MS)
            flushDownloadNotifications()
        }
    }

    private fun flushDownloadNotifications() {
        val pending: List<Pair<String, DownloadCompletedData>>
        synchronized(pendingDownloadNotifications) {
            pending = pendingDownloadNotifications.toList()
            pendingDownloadNotifications.clear()
        }
        if (pending.isNotEmpty()) {
            val notificationIds = pending.map { it.first }
            val downloads = pending.map { it.second }
            systemNotificationHelper.showDownloadsCompletedNotification(downloads, notificationIds)
            logger.debug("Showed grouped download notification for ${downloads.size} album(s)")
        }
    }

    /**
     * Show a single grouped system notification for unread download completions received
     * during full sync. During full sync, notifications are loaded into the in-app list but
     * system notifications are not shown (unlike catch-up/WebSocket paths which go through
     * applyStoredEvent). Only includes notifications created within the last 24 hours to
     * avoid surfacing stale notifications after app reinstall or data clear.
     */
    internal fun showSystemNotificationsForUnread(notifications: List<com.lelloman.pezzottify.android.domain.notifications.Notification>) {
        val cutoffMs = System.currentTimeMillis() - NOTIFICATION_RECENCY_WINDOW_MS
        val notificationIds = mutableListOf<String>()
        val downloads = mutableListOf<DownloadCompletedData>()

        for (notification in notifications) {
            if (notification.readAt != null) continue
            if (notification.notificationType != NotificationType.DownloadCompleted) continue
            if (notification.createdAt < cutoffMs) continue

            try {
                downloads.add(
                    kotlinx.serialization.json.Json.decodeFromJsonElement(
                        DownloadCompletedData.serializer(),
                        notification.data
                    )
                )
                notificationIds.add(notification.id)
            } catch (e: Exception) {
                logger.error("Failed to parse download completed notification data during full sync", e)
            }
        }

        if (downloads.isNotEmpty()) {
            systemNotificationHelper.showDownloadsCompletedNotification(downloads, notificationIds)
            logger.debug("Showed grouped system notification for ${downloads.size} unread download(s)")
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

    private suspend fun applySettingsState(settings: List<UserSetting>) {
        settings.forEach { setting ->
            applySetting(setting)
        }
        logger.debug("Applied ${settings.size} settings")
    }

    private suspend fun applyPlaylistsState(playlists: List<PlaylistState>) {
        // Collect locally-pending playlists before replacing
        val pendingPlaylists = userPlaylistStore.getPendingSyncPlaylists().first()
        val pendingById = pendingPlaylists.associateBy { it.id }

        val serverPlaylists = playlists.map { playlist ->
            // If a local version is pending sync, preserve the local version
            val pending = pendingById[playlist.id]
            if (pending != null) {
                pending
            } else {
                object : com.lelloman.pezzottify.android.domain.usercontent.UserPlaylist {
                    override val id = playlist.id
                    override val name = playlist.name
                    override val trackIds = playlist.tracks
                    override val syncStatus = com.lelloman.pezzottify.android.domain.usercontent.PlaylistSyncStatus.Synced
                }
            }
        }

        // Also include pending playlists that don't exist on the server (e.g. PendingCreate)
        val serverIds = playlists.map { it.id }.toSet()
        val localOnlyPending = pendingPlaylists.filter { it.id !in serverIds }

        userPlaylistStore.replaceAllPlaylists(serverPlaylists + localOnlyPending)
        logger.debug("Applied ${playlists.size} playlists (preserved ${pendingPlaylists.size} pending)")
    }

    private suspend fun applySetting(setting: UserSetting) {
        when (setting) {
            is UserSetting.NotifyWhatsNew -> {
                userSettingsStore.setNotifyWhatsNewEnabled(setting.value)
            }
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

    private fun scheduleRetry() {
        // Cancel any existing retry job
        retryJob?.cancel()

        // Skip scheduling if retries are disabled (infinite delay)
        if (!currentRetryDelay.isFinite()) {
            logger.info("Retries disabled, not scheduling")
            return
        }

        retryJob = scope.launch {
            logger.info("Scheduling retry in $currentRetryDelay")
            delay(currentRetryDelay)

            // Increase delay for next retry (exponential backoff)
            currentRetryDelay = (currentRetryDelay * BACKOFF_MULTIPLIER).coerceAtMost(maxRetryDelay)

            logger.info("Retrying sync...")
            initialize()
        }
    }

    private fun cancelRetry() {
        retryJob?.cancel()
        retryJob = null
        currentRetryDelay = minRetryDelay
    }

    /**
     * Process pending notification reads that were queued while offline.
     * This is called after successful fullSync or catchUp.
     */
    private suspend fun processPendingNotificationReads() {
        if (notificationRepository.hasPendingReads()) {
            logger.info("Processing pending notification reads...")
            val count = notificationRepository.processPendingReads()
            logger.info("Processed $count pending notification reads")
        }
    }

    companion object {
        private val MIN_RETRY_DELAY = 5.seconds
        private val MAX_RETRY_DELAY = 5.minutes
        private const val BACKOFF_MULTIPLIER = 2.0
        private const val NOTIFICATION_RECENCY_WINDOW_MS = 24 * 60 * 60 * 1000L // 24 hours
        private const val DOWNLOAD_NOTIFICATION_DEBOUNCE_MS = 2000L // 2 seconds
    }
}
