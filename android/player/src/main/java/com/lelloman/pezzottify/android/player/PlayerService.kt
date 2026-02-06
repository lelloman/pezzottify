package com.lelloman.pezzottify.android.player

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Bitmap
import android.media.AudioManager
import androidx.annotation.OptIn
import androidx.core.content.ContextCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaMetadata
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import coil3.ImageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.TrackMetadata
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.remoteapi.internal.OkHttpClientFactory
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
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

    @Inject
    lateinit var playbackMetadataProvider: PlaybackMetadataProvider

    @Inject
    lateinit var imageLoader: ImageLoader

    @Inject
    lateinit var loggerFactory: LoggerFactory

    private val logger by lazy { loggerFactory.getLogger("PlaybackService") }

    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var metadataObserverJob: Job? = null
    private var artworkLoadJob: Job? = null
    private var currentArtworkUrl: String? = null
    private var currentArtworkTrackId: String? = null
    private var currentArtworkBytes: ByteArray? = null

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

    // Track whether we have content loaded - used to keep foreground service alive
    private var hasContent = false

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
        startObservingMetadata()
    }

    private fun startObservingMetadata() {
        metadataObserverJob?.cancel()
        metadataObserverJob = serviceScope.launch {
            playbackMetadataProvider.queueState.collectLatest { queueState ->
                val currentTrack = queueState?.currentTrack
                if (currentTrack != null) {
                    updateMediaSessionMetadata(currentTrack)
                } else {
                    clearMediaSessionMetadata()
                }
            }
        }
    }

    private fun updateMediaSessionMetadata(track: TrackMetadata) {
        logger.info("Updating media session metadata: trackId=${track.trackId}, name=${track.trackName}")

        val metadataBuilder = MediaMetadata.Builder()
            .setTitle(track.trackName)
            .setArtist(track.artistNames.joinToString(", "))
            .setAlbumTitle(track.albumName)

        val artworkUrl = track.artworkUrl

        // Reuse cached artwork if URL hasn't changed
        val cachedBytes = currentArtworkBytes
        if (artworkUrl != null && artworkUrl == currentArtworkUrl && cachedBytes != null) {
            metadataBuilder.setArtworkData(cachedBytes, MediaMetadata.PICTURE_TYPE_FRONT_COVER)
        }

        // Update the current MediaItem with new metadata
        updateCurrentMediaItemMetadata(metadataBuilder.build())

        // Load artwork asynchronously if URL changed
        if (artworkUrl != null && artworkUrl != currentArtworkUrl) {
            currentArtworkUrl = artworkUrl
            currentArtworkTrackId = track.trackId
            currentArtworkBytes = null
            loadArtworkAsync(artworkUrl, track)
        }
    }

    @OptIn(UnstableApi::class)
    private fun updateCurrentMediaItemMetadata(metadata: MediaMetadata) {
        val exoPlayer = player ?: return
        val currentIndex = exoPlayer.currentMediaItemIndex
        if (currentIndex < 0 || currentIndex >= exoPlayer.mediaItemCount) return

        val currentItem = exoPlayer.getMediaItemAt(currentIndex)
        val updatedItem = currentItem.buildUpon()
            .setMediaMetadata(metadata)
            .build()

        exoPlayer.replaceMediaItem(currentIndex, updatedItem)
    }

    private fun loadArtworkAsync(artworkUrl: String, track: TrackMetadata) {
        artworkLoadJob?.cancel()
        artworkLoadJob = serviceScope.launch(Dispatchers.IO) {
            logger.debug("Loading artwork for trackId=${track.trackId} from: $artworkUrl")
            try {
                val request = ImageRequest.Builder(this@PlaybackService)
                    .data(artworkUrl)
                    .allowHardware(false) // Required to get a software bitmap
                    .build()

                val result = imageLoader.execute(request)
                if (result is SuccessResult) {
                    val bitmap = result.image.toBitmap()
                    logger.debug("Artwork loaded for trackId=${track.trackId}: ${bitmap.width}x${bitmap.height}")

                    // Validate that we're still on the same track before applying artwork
                    if (currentArtworkTrackId == track.trackId) {
                        updateMediaSessionWithArtwork(track, bitmap)
                    } else {
                        logger.warn("Artwork discarded: expected trackId=${track.trackId} but current is $currentArtworkTrackId")
                    }
                } else {
                    logger.warn("Failed to load artwork for trackId=${track.trackId}: $result")
                }
            } catch (e: Exception) {
                logger.error("Error loading artwork for trackId=${track.trackId}", e)
            }
        }
    }

    private fun updateMediaSessionWithArtwork(track: TrackMetadata, artwork: Bitmap) {
        serviceScope.launch(Dispatchers.Main) {
            val artworkBytes = bitmapToByteArray(artwork)
            currentArtworkBytes = artworkBytes

            val metadata = MediaMetadata.Builder()
                .setTitle(track.trackName)
                .setArtist(track.artistNames.joinToString(", "))
                .setAlbumTitle(track.albumName)
                .setArtworkData(artworkBytes, MediaMetadata.PICTURE_TYPE_FRONT_COVER)
                .build()

            updateCurrentMediaItemMetadata(metadata)
            logger.info("Media session updated with artwork for trackId=${track.trackId}")
        }
    }

    private fun bitmapToByteArray(bitmap: Bitmap): ByteArray {
        val stream = java.io.ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream)
        return stream.toByteArray()
    }

    private fun clearMediaSessionMetadata() {
        currentArtworkUrl = null
        currentArtworkTrackId = null
        currentArtworkBytes = null
        artworkLoadJob?.cancel()
        // Clear metadata by updating with empty metadata
        updateCurrentMediaItemMetadata(MediaMetadata.EMPTY)
        logger.debug("Media session metadata cleared")
    }

    private fun releasePlayerAndSession() {
        player?.let {
            it.playWhenReady = false
            it.release()
            player = null
        }
        mediaSession?.let {
            it.release()
            mediaSession = null
        }
    }

    override fun onDestroy() {
        metadataObserverJob?.cancel()
        artworkLoadJob?.cancel()
        serviceScope.cancel()
        unregisterReceiver(becomingNoisyReceiver)
        releasePlayerAndSession()
        super.onDestroy()
    }

    @OptIn(UnstableApi::class)
    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        releasePlayerAndSession()
        playerServiceEventsEmitter.shutdown()
        pauseAllPlayersAndStopSelf()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        mediaSession

    /**
     * Override notification handling to keep the foreground service alive when paused.
     *
     * By default, MediaSessionService stops the foreground service when playback pauses,
     * which can lead to the service being killed by the system. This override ensures
     * the service stays in the foreground as long as there's content loaded.
     */
    @OptIn(UnstableApi::class)
    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        val exoPlayer = player ?: return

        // Update hasContent based on whether there are media items loaded
        hasContent = exoPlayer.mediaItemCount > 0

        if (!hasContent) {
            // No content - let Media3 handle default behavior (will stop foreground)
            super.onUpdateNotification(session, startInForegroundRequired)
            return
        }

        // We have content - always keep the service in foreground
        // This prevents Android from killing the service while paused
        super.onUpdateNotification(session, true)
    }
}