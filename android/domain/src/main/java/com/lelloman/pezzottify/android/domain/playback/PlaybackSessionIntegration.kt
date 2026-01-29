package com.lelloman.pezzottify.android.domain.playback

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Integrates the local player with the remote playback session.
 *
 * When playback starts locally with a track, registers this device as the audio device.
 * This enables other devices to see and control the playback session.
 */
@Singleton
class PlaybackSessionIntegration @Inject constructor(
    private val player: PezzottifyPlayer,
    private val remotePlaybackManager: RemotePlaybackManager,
    loggerFactory: LoggerFactory,
    @IoDispatcher dispatcher: CoroutineDispatcher,
) : AppInitializer {

    private val logger = loggerFactory.getLogger(PlaybackSessionIntegration::class)
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)

    private var wasPlayingWithTrack = false

    override fun initialize() {
        logger.info("Initializing playback session integration")

        scope.launch {
            // Combine playing state, playlist, and session state
            combine(
                player.isPlaying,
                player.playbackPlaylist,
                remotePlaybackManager.sessionExists,
                remotePlaybackManager.isAudioDevice,
            ) { isPlaying, playlist, sessionExists, isAudioDevice ->
                val hasTrack = playlist != null && playlist.tracksIds.isNotEmpty()
                PlaybackSessionState(isPlaying, hasTrack, sessionExists, isAudioDevice)
            }.collectLatest { state ->
                val isPlayingWithTrack = state.isPlaying && state.hasTrack
                if (isPlayingWithTrack && !wasPlayingWithTrack) {
                    // Playback started with a track
                    if (!state.sessionExists || !state.isAudioDevice) {
                        logger.info("Playback started with track, registering as audio device")
                        remotePlaybackManager.registerAsAudioDevice()
                    }
                }
                wasPlayingWithTrack = isPlayingWithTrack
            }
        }
    }

    private data class PlaybackSessionState(
        val isPlaying: Boolean,
        val hasTrack: Boolean,
        val sessionExists: Boolean,
        val isAudioDevice: Boolean,
    )
}
