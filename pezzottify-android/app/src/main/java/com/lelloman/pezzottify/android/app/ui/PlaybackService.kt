package com.lelloman.pezzottify.android.app.ui

import android.app.PendingIntent
import android.content.Intent
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.lelloman.pezzottify.android.app.player.PlayerManager
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.runBlocking
import javax.inject.Inject

@AndroidEntryPoint
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {

    @Inject
    lateinit var playerManager: PlayerManager
    private var mediaSession: MediaSession? = null

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo) = mediaSession

    override fun onCreate() {
        super.onCreate()
        val activityIntent = Intent(this, MainActivity::class.java)
            .setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT)
        val pendingIntent = PendingIntent.getActivity(
            this,
            123,
            activityIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        mediaSession = MediaSession.Builder(this, playerManager.getPlayer())
            .setSessionActivity(pendingIntent)
            .build()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        val shouldStop = runBlocking {
            playerManager.withPlayer { !it.playWhenReady || it.mediaItemCount == 0 }
        }
        if (shouldStop) stopSelf()
    }

    override fun onDestroy() {
        mediaSession?.run {
            runBlocking { playerManager.dispose() }
            release()
            mediaSession = null
        }
        super.onDestroy()
    }
}