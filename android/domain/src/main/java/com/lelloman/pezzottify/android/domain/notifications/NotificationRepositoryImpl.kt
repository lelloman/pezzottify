package com.lelloman.pezzottify.android.domain.notifications

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Implementation of NotificationRepository that maintains in-memory notification state
 * and broadcasts updates via flows.
 */
@Singleton
class NotificationRepositoryImpl @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    loggerFactory: LoggerFactory,
) : NotificationRepository {

    private val logger: Logger by loggerFactory

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private val _notifications = MutableStateFlow<List<Notification>>(emptyList())
    override val notifications: StateFlow<List<Notification>> = _notifications.asStateFlow()

    override val unreadCount: StateFlow<Int> = _notifications
        .map { list -> list.count { it.readAt == null } }
        .stateIn(scope, SharingStarted.Eagerly, 0)

    private val _updates = MutableSharedFlow<NotificationUpdate>(extraBufferCapacity = 64)
    override fun observeUpdates(): Flow<NotificationUpdate> = _updates.asSharedFlow()

    override suspend fun setNotifications(notifications: List<Notification>) {
        logger.debug("setNotifications() count=${notifications.size}")
        _notifications.value = notifications.sortedByDescending { it.createdAt }
    }

    override suspend fun onNotificationCreated(notification: Notification) {
        logger.debug("onNotificationCreated() id=${notification.id} title=${notification.title}")
        _notifications.update { current ->
            (listOf(notification) + current).take(MAX_NOTIFICATIONS)
        }
        _updates.emit(NotificationUpdate.Created(notification))
    }

    override suspend fun onNotificationRead(notificationId: String, readAt: Long) {
        logger.debug("onNotificationRead() id=$notificationId readAt=$readAt")
        _notifications.update { current ->
            current.map {
                if (it.id == notificationId) it.copy(readAt = readAt) else it
            }
        }
        _updates.emit(NotificationUpdate.Read(notificationId, readAt))
    }

    override suspend fun markAsRead(notificationId: String): Result<Unit> {
        logger.debug("markAsRead() id=$notificationId")
        return when (val response = remoteApiClient.markNotificationRead(notificationId)) {
            is RemoteApiResponse.Success -> {
                // Local update happens via sync event
                Result.success(Unit)
            }
            is RemoteApiResponse.Error -> {
                logger.error("Failed to mark notification as read: $response")
                Result.failure(Exception("Failed to mark notification as read"))
            }
        }
    }

    override suspend fun clear() {
        logger.debug("clear()")
        _notifications.value = emptyList()
    }

    companion object {
        private const val MAX_NOTIFICATIONS = 100
    }
}
