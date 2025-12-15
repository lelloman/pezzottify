package com.lelloman.pezzottify.android.player

import android.app.PendingIntent
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
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.remoteapi.internal.OkHttpClientFactory
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class PlaybackService : MediaSessionService() {

    @Inject
    lateinit var authStore: AuthStore

    @Inject
    lateinit var configStore: ConfigStore

    @Inject
    lateinit var okHttpClientFactory: OkHttpClientFactory

    @Inject
    internal lateinit var playerServiceEventsEmitter: PlayerServiceEventsEmitter

    private val authToken get() = (authStore.getAuthState().value as? AuthState.LoggedIn)?.authToken.orEmpty()

    private val okHttpClient by lazy {
        okHttpClientFactory.createBuilder(configStore.baseUrl.value)
            .addInterceptor {
                it.proceed(
                    it.request().newBuilder()
                        .addHeader("Authorization", authToken)
                        .build()
                )
            }
            .build()
    }

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

    @OptIn(UnstableApi::class)
    override fun onCreate() {
        super.onCreate()
        setMediaNotificationProvider(
            DefaultMediaNotificationProvider.Builder(this)
                .setChannelId(MEDIA_PLAYBACK_CHANNEL_ID)
                .setChannelName(R.string.media_notification_channel_name)
                .build()
        )
        mediaSession = MediaSession.Builder(this, makePlayer())
            .setSessionActivity(createSessionActivityPendingIntent())
            .build()
        ContextCompat.registerReceiver(
            this,
            becomingNoisyReceiver,
            IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
    }

    companion object {
        const val MEDIA_PLAYBACK_CHANNEL_ID = "pezzottify_media_playback"
    }

    private fun createSessionActivityPendingIntent(): PendingIntent {
        val intent = packageManager.getLaunchIntentForPackage(packageName)
            ?: Intent().apply {
                setPackage(packageName)
                action = Intent.ACTION_MAIN
                addCategory(Intent.CATEGORY_LAUNCHER)
            }
        intent.flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        return PendingIntent.getActivity(
            this,
            0,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
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