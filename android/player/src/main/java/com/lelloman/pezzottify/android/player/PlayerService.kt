package com.lelloman.pezzottify.android.player

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioManager
import androidx.annotation.OptIn
import androidx.core.content.ContextCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import dagger.hilt.android.AndroidEntryPoint
import okhttp3.OkHttpClient
import javax.inject.Inject

@AndroidEntryPoint
class PlaybackService : MediaSessionService() {

    @Inject
    lateinit var authStore: AuthStore

    @Inject
    internal lateinit var playerServiceEventsEmitter: PlayerServiceEventsEmitter

    private val authToken get() = (authStore.getAuthState().value as? AuthState.LoggedIn)?.authToken.orEmpty()
    private val okHttpClient = OkHttpClient.Builder()
        .addInterceptor {
            it.proceed(
                it.request().newBuilder()
                    .addHeader("Authorization", authToken)
                    .build()
            )
        }
        .build()

    private var mediaSession: MediaSession? = null

    private var player: ExoPlayer? = null

    private val becomingNoisyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) {
                player?.pause()
            }
        }
    }

    @OptIn(UnstableApi::class)
    private fun makePlayer(): ExoPlayer = ExoPlayer
        .Builder(this).setMediaSourceFactory(
            DefaultMediaSourceFactory(this).setDataSourceFactory(
                DefaultDataSource.Factory(
                    this,
                    OkHttpDataSource.Factory { okHttpClient.newCall(it) })
            )
        )
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(C.USAGE_MEDIA)
                .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                .build(),
            /* handleAudioFocus = */ true
        )
        .build()
        .apply { player = this }

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

    override fun onDestroy() {
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
        mediaSession?.release()
        mediaSession = null
        player?.playWhenReady = false
        player?.release()
        player = null

        playerServiceEventsEmitter.shutdown()
        pauseAllPlayersAndStopSelf()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        mediaSession
}