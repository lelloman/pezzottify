package com.lelloman.pezzottify.android.localplayer

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioManager
import androidx.annotation.OptIn
import androidx.core.content.ContextCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * MediaSessionService for local audio file playback.
 * Provides notification controls and lock screen integration.
 * No server dependencies - purely for local files.
 */
class LocalPlaybackService : MediaSessionService() {

    private var mediaSession: MediaSession? = null
    private var player: ExoPlayer? = null

    private val becomingNoisyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) {
                player?.pause()
            }
        }
    }

    private val playerListener = object : Player.Listener {
        @OptIn(UnstableApi::class)
        override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
            // Update metadata when track changes
            mediaItem?.let { updateMetadataFromMediaItem(it) }
        }
    }

    @OptIn(UnstableApi::class)
    private fun makePlayer(): ExoPlayer = ExoPlayer
        .Builder(this)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(C.USAGE_MEDIA)
                .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                .build(),
            /* handleAudioFocus = */ true
        )
        .build()
        .also {
            player = it
            it.addListener(playerListener)
        }

    override fun onCreate() {
        super.onCreate()
        mediaSession = MediaSession.Builder(this, makePlayer()).build()
        ContextCompat.registerReceiver(
            this,
            becomingNoisyReceiver,
            IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
    }

    @OptIn(UnstableApi::class)
    private fun updateMetadataFromMediaItem(mediaItem: MediaItem) {
        val exoPlayer = player ?: return
        val currentIndex = exoPlayer.currentMediaItemIndex
        if (currentIndex < 0 || currentIndex >= exoPlayer.mediaItemCount) return

        // Extract display name from the URI or existing metadata
        val displayName = mediaItem.mediaMetadata.title
            ?: mediaItem.localConfiguration?.uri?.lastPathSegment
            ?: "Unknown"

        val metadata = MediaMetadata.Builder()
            .setTitle(displayName)
            .setArtist("Local File")
            .build()

        val updatedItem = mediaItem.buildUpon()
            .setMediaMetadata(metadata)
            .build()

        exoPlayer.replaceMediaItem(currentIndex, updatedItem)
    }

    override fun onDestroy() {
        player?.removeListener(playerListener)
        unregisterReceiver(becomingNoisyReceiver)
        mediaSession?.run {
            player.release()
            release()
            mediaSession = null
        }
        super.onDestroy()
    }

    @OptIn(UnstableApi::class)
    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        // Save state before stopping (will be implemented)
        savePlaybackState()

        mediaSession?.player?.let { player ->
            if (!player.playWhenReady || player.mediaItemCount == 0) {
                stopSelf()
            }
        }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        mediaSession

    @OptIn(UnstableApi::class)
    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        val exoPlayer = player ?: return

        val hasContent = exoPlayer.mediaItemCount > 0
        if (!hasContent) {
            super.onUpdateNotification(session, startInForegroundRequired)
            return
        }

        // Keep service in foreground as long as there's content
        super.onUpdateNotification(session, true)
    }

    private fun savePlaybackState() {
        val exoPlayer = player ?: return
        val prefs = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

        // Build queue URIs
        val queueUris = (0 until exoPlayer.mediaItemCount).mapNotNull { index ->
            exoPlayer.getMediaItemAt(index).localConfiguration?.uri?.toString()
        }

        prefs.edit()
            .putString(KEY_QUEUE_URIS, queueUris.joinToString(SEPARATOR))
            .putInt(KEY_CURRENT_INDEX, exoPlayer.currentMediaItemIndex)
            .putLong(KEY_POSITION_MS, exoPlayer.currentPosition)
            .putLong(KEY_SAVED_AT, System.currentTimeMillis())
            .apply()
    }

    companion object {
        const val PREFS_NAME = "local_player_state"
        const val KEY_QUEUE_URIS = "queue_uris"
        const val KEY_CURRENT_INDEX = "current_index"
        const val KEY_POSITION_MS = "position_ms"
        const val KEY_SAVED_AT = "saved_at"
        const val SEPARATOR = "\n"
    }
}
