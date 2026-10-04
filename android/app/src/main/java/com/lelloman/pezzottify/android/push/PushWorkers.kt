package com.lelloman.pezzottify.android.push

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.work.CoroutineWorker
import androidx.work.ForegroundInfo
import androidx.work.WorkerParameters
import com.lelloman.pezzottify.android.R
import com.lelloman.pezzottify.android.sync.BackgroundSyncWorker
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent

@EntryPoint
@InstallIn(SingletonComponent::class)
interface PushEntryPoint {
    fun unifiedPushRegistration(): UnifiedPushRegistration
}

internal fun Context.pushRegistration(): UnifiedPushRegistration =
    EntryPointAccessors.fromApplication(applicationContext, PushEntryPoint::class.java)
        .unifiedPushRegistration()

/** Sends or removes this device's push registration on the server, with retries. */
class PushServerSyncWorker(
    appContext: Context,
    params: WorkerParameters,
) : CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        val registration = applicationContext.pushRegistration()
        val outcome = when (inputData.getString(KEY_OPERATION)) {
            OP_REGISTER -> registration.registerOnServer()
            OP_DELETE -> inputData.getString(KEY_ENDPOINT)
                ?.let { registration.deleteOnServer(it) }
                ?: ServerSyncResult.Done
            else -> ServerSyncResult.Done
        }
        return when (outcome) {
            ServerSyncResult.Done -> Result.success()
            ServerSyncResult.Retry -> if (runAttemptCount < MAX_ATTEMPTS) Result.retry() else Result.failure()
        }
    }

    companion object {
        const val KEY_OPERATION = "operation"
        const val KEY_ENDPOINT = "endpoint"
        const val OP_REGISTER = "register"
        const val OP_DELETE = "delete"
        private const val MAX_ATTEMPTS = 20
    }
}

/**
 * Sync catch-up triggered by a push wake-up. Same work as the periodic background sync,
 * run as expedited work; below API 31 expedited work needs foreground info.
 */
class PushSyncWorker(
    appContext: Context,
    params: WorkerParameters,
) : BackgroundSyncWorker(appContext, params) {

    override suspend fun getForegroundInfo(): ForegroundInfo {
        val manager = applicationContext.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    applicationContext.getString(R.string.notification_channel_sync_name),
                    NotificationManager.IMPORTANCE_MIN,
                )
            )
        }
        val notification = NotificationCompat.Builder(applicationContext, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(applicationContext.getString(R.string.notification_sync_title))
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .setSilent(true)
            .build()
        // Only used below API 31, where expedited work runs as an untyped foreground service.
        return ForegroundInfo(NOTIFICATION_ID, notification)
    }

    private companion object {
        const val CHANNEL_ID = "sync"
        const val NOTIFICATION_ID = 0x5059
    }
}
