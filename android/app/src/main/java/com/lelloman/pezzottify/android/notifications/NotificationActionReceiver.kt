package com.lelloman.pezzottify.android.notifications

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepository
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * BroadcastReceiver that marks internal notifications as read when the user
 * dismisses (swipes away) a system notification.
 */
@AndroidEntryPoint
class NotificationActionReceiver : BroadcastReceiver() {

    @Inject
    lateinit var notificationRepository: NotificationRepository

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION_NOTIFICATION_DISMISSED) return

        val notificationIds = intent.getStringArrayExtra(EXTRA_NOTIFICATION_IDS) ?: return
        if (notificationIds.isEmpty()) return

        val pendingResult = goAsync()
        scope.launch {
            try {
                for (id in notificationIds) {
                    notificationRepository.markAsRead(id)
                }
            } finally {
                pendingResult.finish()
            }
        }
    }

    companion object {
        const val ACTION_NOTIFICATION_DISMISSED = "com.lelloman.pezzottify.NOTIFICATION_DISMISSED"
        const val EXTRA_NOTIFICATION_IDS = "notification_ids"
    }
}
