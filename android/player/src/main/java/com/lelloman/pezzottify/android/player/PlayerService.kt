package com.lelloman.pezzottify.android.player

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Bitmap
import android.media.AudioManager
import android.os.Looper
import androidx.annotation.OptIn
import androidx.core.content.ContextCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
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
import com.lelloman.pezzottify.android.domain.auth.SessionExpiredHandler
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.auth.bearerAuthorization
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.equalizer.EqualizerStore
import com.lelloman.pezzottify.android.player.equalizer.EqualizerRenderersFactory
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.TrackMetadata
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.remoteapi.internal.OkHttpClientFactory
import com.lelloman.pezzottify.android.remoteapi.internal.SessionExpiredInterceptor
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * The one foreground service behind every media control on this device.
 *
 * It hosts the local session (ExoPlayer) and, while controlling another device, a second session
 * whose player mirrors that device. The remote session is separate rather than swapped into the
 * local one because [ExoPlatformPlayer] stays connected to the local session, and would otherwise
 * read the remote device's state as local playback.
 */
@AndroidEntryPoint
class PlaybackService : MediaSessionService() {

    @Inject
    lateinit var equalizerStore: EqualizerStore

    @Inject
    lateinit var equalizerOutputs: com.lelloman.pezzottify.android.player.equalizer.AndroidEqualizerOutputController

    @Inject
    lateinit var authStore: AuthStore

    @Inject
    lateinit var configStore: ConfigStore

    @Inject
    lateinit var okHttpClientFactory: OkHttpClientFactory

    @Inject
    lateinit var tokenRefresher: TokenRefresher

    @Inject
    lateinit var sessionExpiredHandler: SessionExpiredHandler

    @Inject
    internal lateinit var playerServiceEventsEmitter: PlayerServiceEventsEmitter

    @Inject
    lateinit var playbackMetadataProvider: PlaybackMetadataProvider

    @Inject
    lateinit var imageLoader: ImageLoader

    @Inject
    lateinit var playbackModeManager: PlaybackModeManager

    @Inject
    lateinit var routedPlayer: PezzottifyPlayer

    @Inject
    lateinit var loggerFactory: LoggerFactory

    private val logger by lazy { loggerFactory.getLogger("PlaybackService") }
    private val diagnostics by lazy { PlaybackDiagnostics(loggerFactory, "service") }

    private val sessionCallback = object : MediaSession.Callback {
        override fun onPostConnect(session: MediaSession, controller: MediaSession.ControllerInfo) {
            diagnostics.record("session_controller_connected caller=${controller.packageName}", player)
            super.onPostConnect(session, controller)
        }

        override fun onDisconnected(session: MediaSession, controller: MediaSession.ControllerInfo) {
            diagnostics.record("session_controller_disconnected caller=${controller.packageName}", player)
            super.onDisconnected(session, controller)
        }

        @androidx.media3.common.util.UnstableApi
        override fun onPlayerCommandRequest(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
            playerCommand: Int,
        ): Int {
            if (playerCommand == Player.COMMAND_STOP ||
                playerCommand == Player.COMMAND_PLAY_PAUSE ||
                playerCommand == Player.COMMAND_PREPARE) {
                diagnostics.record("controller_command command=$playerCommand caller=${controller.packageName}", player)
            }
            return super.onPlayerCommandRequest(session, controller, playerCommand)
        }
    }

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
                        .addHeader("Authorization", bearerAuthorization(authToken))
                        .build()
                )
            }
            .addInterceptor(
                SessionExpiredInterceptor(sessionExpiredHandler, tokenRefresher, logger)
            )
            .build()
    }

    private var mediaSession: MediaSession? = null

    private var player: ExoPlayer? = null

    private var remoteSession: MediaSession? = null
    private var remotePlayer: RemoteSessionPlayer? = null
    private var remoteSnapshotsJob: Job? = null

    private val becomingNoisyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) {
                diagnostics.record("becoming_noisy_pause", player)
                player?.pause()
            }
        }
    }

    @OptIn(UnstableApi::class)
    private fun makePlayer(): ExoPlayer = ExoPlayer
        .Builder(this, EqualizerRenderersFactory(this, equalizerStore, equalizerOutputs)).setMediaSourceFactory(
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
        .apply {
            player = this
            addListener(diagnostics.listener(this))
            diagnostics.record("player_created", this)
        }

    override fun onCreate() {
        super.onCreate()
        diagnostics.record("service_created")
        mediaSession = MediaSession.Builder(this, makePlayer()).setCallback(sessionCallback).build()
        ContextCompat.registerReceiver(
            this,
            becomingNoisyReceiver,
            IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
        startObservingMetadata()
        startObservingPlaybackMode()
    }

    private fun startObservingPlaybackMode() {
        serviceScope.launch {
            playbackModeManager.mode.collect { mode ->
                when (mode) {
                    is PlaybackMode.Remote -> startRemoteSession(mode.deviceName)
                    is PlaybackMode.Local -> releaseRemoteSession()
                }
            }
        }
    }

    @OptIn(UnstableApi::class)
    private fun startRemoteSession(deviceName: String) {
        releaseRemoteSession()
        val remote = RemoteSessionPlayer(Looper.getMainLooper(), routedPlayer, deviceName)
        currentArtworkTrackId?.let { trackId ->
            currentArtworkBytes?.let { remote.setArtwork(trackId, it) }
        }
        val session = MediaSession.Builder(this, remote).setId(REMOTE_SESSION_ID).build()
        remotePlayer = remote
        remoteSession = session
        addSession(session)
        remoteSnapshotsJob = serviceScope.launch {
            remoteSessionSnapshots(deviceName, routedPlayer, playbackMetadataProvider.queueState)
                .collect { remote.update(it) }
        }
    }

    private fun releaseRemoteSession() {
        remoteSnapshotsJob?.cancel()
        remoteSnapshotsJob = null
        remoteSession?.let {
            removeSession(it)
            it.release()
        }
        remoteSession = null
        remotePlayer?.release()
        remotePlayer = null
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
            remotePlayer?.setArtwork(track.trackId, cachedBytes)
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
            remotePlayer?.setArtwork(track.trackId, artworkBytes)

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
        diagnostics.record("release_player_and_session", player)
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
        diagnostics.record("service_destroy", player)
        metadataObserverJob?.cancel()
        artworkLoadJob?.cancel()
        playerServiceEventsEmitter.shutdown()
        serviceScope.cancel()
        unregisterReceiver(becomingNoisyReceiver)
        releaseRemoteSession()
        releasePlayerAndSession()
        super.onDestroy()
    }

    @OptIn(UnstableApi::class)
    override fun onTaskRemoved(rootIntent: Intent?) {
        diagnostics.record("task_removed", player)
        super.onTaskRemoved(rootIntent)
        // Closing the app on this device must not stop music on the other one, so unlike
        // leaving the controller from the UI, this does not pause the remote device.
        playbackModeManager.exitRemoteMode()
        releaseRemoteSession()
        releasePlayerAndSession()
        playerServiceEventsEmitter.shutdown()
        pauseAllPlayersAndStopSelf()
    }

    /** Our own controller only ever drives local playback; see the class comment. */
    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        if (controllerInfo.packageName == packageName) mediaSession else remoteSession ?: mediaSession

    /**
     * Keeps the service in the foreground for as long as a session has content, paused or not.
     *
     * By default, MediaSessionService leaves the foreground when playback pauses, and the system
     * may then kill the process along with the session and the WebSocket keeping it reachable.
     * This covers the remote session too: it always has at least a placeholder item.
     */
    @OptIn(UnstableApi::class)
    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        val hasContent = session.player.mediaItemCount > 0
        super.onUpdateNotification(session, startInForegroundRequired || hasContent)
    }

    private companion object {
        const val REMOTE_SESSION_ID = "remote-control"
    }
}
