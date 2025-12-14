package com.lelloman.pezzottify.android.ui.screen.main.notifications

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * UI state for the notification list screen.
 */
data class NotificationListScreenState(
    val notifications: List<UiNotification> = emptyList(),
)

/**
 * UI model for a notification.
 */
data class UiNotification(
    val id: String,
    val title: String,
    val body: String?,
    val readAt: Long?,
    val createdAt: Long,
    val relativeTime: String,
    val albumId: String?,
)

@HiltViewModel
class NotificationListScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    val state: StateFlow<NotificationListScreenState> = interactor.getNotifications()
        .map { notifications ->
            NotificationListScreenState(notifications = notifications)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), NotificationListScreenState())

    fun markAsRead(notificationId: String) {
        viewModelScope.launch {
            interactor.markAsRead(notificationId)
        }
    }

    interface Interactor {
        fun getNotifications(): Flow<List<UiNotification>>
        suspend fun markAsRead(notificationId: String)
    }
}
