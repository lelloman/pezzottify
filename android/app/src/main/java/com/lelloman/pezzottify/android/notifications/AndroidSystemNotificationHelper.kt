package com.lelloman.pezzottify.android.notifications

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import com.lelloman.pezzottify.android.MainActivity
import com.lelloman.pezzottify.android.R
import com.lelloman.pezzottify.android.domain.notifications.DownloadCompletedData
import com.lelloman.pezzottify.android.domain.notifications.SystemNotificationHelper
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Android implementation of SystemNotificationHelper.
 * Handles creating notification channels and showing system notifications.
 */
@Singleton
class AndroidSystemNotificationHelper @Inject constructor(
    @ApplicationContext private val context: Context,
) : SystemNotificationHelper {

    private val notificationManager =
        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    init {
        createNotificationChannels()
    }

    private fun createNotificationChannels() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val whatsNewChannel = NotificationChannel(
                WHATSNEW_CHANNEL_ID,
                context.getString(R.string.notification_channel_whatsnew_name),
                NotificationManager.IMPORTANCE_DEFAULT
            ).apply {
                description = context.getString(R.string.notification_channel_whatsnew_description)
            }
            notificationManager.createNotificationChannel(whatsNewChannel)

            val downloadsChannel = NotificationChannel(
                DOWNLOADS_CHANNEL_ID,
                context.getString(R.string.notification_channel_downloads_name),
                NotificationManager.IMPORTANCE_DEFAULT
            ).apply {
                description = context.getString(R.string.notification_channel_downloads_description)
            }
            notificationManager.createNotificationChannel(downloadsChannel)
        }
    }

    @Suppress("NotificationPermission") // Permission is checked manually before notify()
    override fun showWhatsNewNotification(
        batchId: String,
        batchName: String,
        description: String?,
        albumsAdded: Int,
        artistsAdded: Int,
        tracksAdded: Int,
    ) {
        // Create an intent to open the app and navigate to the What's New screen
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            putExtra(EXTRA_NAVIGATE_TO, DESTINATION_WHATSNEW)
        }

        val pendingIntent = PendingIntent.getActivity(
            context,
            batchId.hashCode(),
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Build the notification content
        val title = context.getString(R.string.notification_whatsnew_title)
        val contentText = buildContentText(batchName, albumsAdded, artistsAdded, tracksAdded)

        val notification = NotificationCompat.Builder(context, WHATSNEW_CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(contentText)
            .setStyle(NotificationCompat.BigTextStyle().bigText(
                description?.let { "$contentText\n$it" } ?: contentText
            ))
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .build()

        // Check notification permission on Android 13+
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(
                    context,
                    Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                // Permission not granted, skip showing notification
                return
            }
        }

        notificationManager.notify(batchId.hashCode(), notification)
    }

    private fun buildContentText(
        batchName: String,
        albumsAdded: Int,
        artistsAdded: Int,
        tracksAdded: Int,
    ): String {
        val parts = mutableListOf<String>()
        if (albumsAdded > 0) {
            parts.add(context.resources.getQuantityString(
                R.plurals.notification_albums_added, albumsAdded, albumsAdded
            ))
        }
        if (artistsAdded > 0) {
            parts.add(context.resources.getQuantityString(
                R.plurals.notification_artists_added, artistsAdded, artistsAdded
            ))
        }
        if (tracksAdded > 0) {
            parts.add(context.resources.getQuantityString(
                R.plurals.notification_tracks_added, tracksAdded, tracksAdded
            ))
        }

        return if (parts.isEmpty()) {
            batchName
        } else {
            "$batchName: ${parts.joinToString(", ")}"
        }
    }

    @Suppress("NotificationPermission") // Permission is checked manually before notify()
    override fun showDownloadCompletedNotification(
        albumId: String,
        albumName: String,
        artistName: String,
    ) {
        // Create an intent to open the app and navigate to the album screen
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            putExtra(EXTRA_NAVIGATE_TO, DESTINATION_ALBUM)
            putExtra(EXTRA_ALBUM_ID, albumId)
        }

        val pendingIntent = PendingIntent.getActivity(
            context,
            albumId.hashCode(),
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val title = context.getString(R.string.notification_download_completed_title)
        val contentText = context.getString(R.string.notification_download_completed_body, albumName, artistName)

        val notification = NotificationCompat.Builder(context, DOWNLOADS_CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(contentText)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .build()

        // Check notification permission on Android 13+
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(
                    context,
                    Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                // Permission not granted, skip showing notification
                return
            }
        }

        notificationManager.notify(albumId.hashCode(), notification)
    }

    @Suppress("NotificationPermission") // Permission is checked manually before notify()
    override fun showDownloadsCompletedNotification(downloads: List<DownloadCompletedData>) {
        if (downloads.isEmpty()) return

        // Single download — delegate to the existing single-album notification
        if (downloads.size == 1) {
            val d = downloads.first()
            showDownloadCompletedNotification(d.albumId, d.albumName, d.artistName)
            return
        }

        // Multiple downloads — show one grouped notification
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }

        val pendingIntent = PendingIntent.getActivity(
            context,
            DOWNLOADS_BATCH_NOTIFICATION_ID,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val title = context.getString(R.string.notification_downloads_completed_title)
        val summary = context.resources.getQuantityString(
            R.plurals.notification_downloads_completed_body, downloads.size, downloads.size
        )
        val details = downloads.joinToString("\n") { "${it.albumName} — ${it.artistName}" }

        val notification = NotificationCompat.Builder(context, DOWNLOADS_CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(summary)
            .setStyle(NotificationCompat.BigTextStyle().bigText("$summary\n$details"))
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .build()

        // Check notification permission on Android 13+
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(
                    context,
                    Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                return
            }
        }

        notificationManager.notify(DOWNLOADS_BATCH_NOTIFICATION_ID, notification)
    }

    companion object {
        const val WHATSNEW_CHANNEL_ID = "whatsnew"
        const val DOWNLOADS_CHANNEL_ID = "downloads"
        const val EXTRA_NAVIGATE_TO = "navigate_to"
        const val EXTRA_ALBUM_ID = "album_id"
        const val DESTINATION_WHATSNEW = "whatsnew"
        const val DESTINATION_ALBUM = "album"
        private const val DOWNLOADS_BATCH_NOTIFICATION_ID = 0x50455A5A // "PEZZ"
    }
}
