package com.lelloman.pezzottify.android

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.os.Build
import com.lelloman.pezzottify.android.domain.usecase.InitializeApp
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class PezzottifyApplication : Application() {

    @Inject
    lateinit var initializeApp: InitializeApp

    @Inject
    lateinit var loggerFactory: LoggerFactory

    override fun onCreate() {
        super.onCreate()
        createNotificationChannels()
        setupUncaughtExceptionHandler()
        initializeApp()
    }

    private fun createNotificationChannels() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                MEDIA_PLAYBACK_CHANNEL_ID,
                "Music Playback",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Shows playback controls for the current track"
                setShowBadge(false)
            }
            val notificationManager = getSystemService(NotificationManager::class.java)
            notificationManager?.createNotificationChannel(channel)
        }
    }

    companion object {
        const val MEDIA_PLAYBACK_CHANNEL_ID = "pezzottify_media_playback"
    }

    private fun setupUncaughtExceptionHandler() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        val logger = loggerFactory.getLogger("UncaughtException")

        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                logger.error("Uncaught exception in thread '${thread.name}'", throwable)
            } catch (_: Exception) {
                // Ignore any logging failures - we don't want to cause another crash
            }
            // Delegate to default handler so the app crashes normally
            defaultHandler?.uncaughtException(thread, throwable)
        }
    }
}