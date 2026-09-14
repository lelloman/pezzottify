package com.lelloman.pezzottify.android.player

import android.os.Process
import android.os.SystemClock
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import com.lelloman.pezzottify.android.logger.LoggerFactory
import java.util.UUID

/**
 * Targeted diagnostic for https://crumbles.lelloman.com/w/LLPR/PEZZOTTIFY/14.
 * Collection/interpretation: docs/diagnostics/android-playback-resume.md.
 * State transitions only: never log media metadata, URIs, exception messages or tokens.
 */
internal class PlaybackDiagnostics(loggerFactory: LoggerFactory, private val owner: String) {
    private val logger = loggerFactory.getLogger("PlaybackDiagnostics")
    private val instance = UUID.randomUUID().toString()

    fun record(event: String, player: Player? = null) {
        val snapshot = player?.let {
            "player=${System.identityHashCode(it)} state=${it.playbackState}" +
                " requested=${it.playWhenReady} playing=${it.isPlaying}" +
                " suppression=${it.playbackSuppressionReason} count=${it.mediaItemCount}" +
                " index=${it.currentMediaItemIndex} positionMs=${it.currentPosition}" +
                " error=${it.playerError?.errorCode}"
        } ?: "player=none"
        logger.info("ticket=LLPR/PEZZOTTIFY-14 run=$processRun pid=${Process.myPid()}" +
            " elapsedMs=${SystemClock.elapsedRealtime()} owner=$owner instance=$instance" +
            " event=$event $snapshot")
    }

    fun listener(player: Player) = object : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) = record("state_changed", player)
        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) =
            record("play_when_ready_changed reason=$reason", player)
        override fun onPlaybackSuppressionReasonChanged(playbackSuppressionReason: Int) =
            record("suppression_changed", player)
        override fun onIsPlayingChanged(isPlaying: Boolean) = record("is_playing_changed", player)
        override fun onPlayerError(error: PlaybackException) =
            record("player_error code=${error.errorCode}", player)
    }

    private companion object {
        val processRun = UUID.randomUUID().toString()
    }
}
