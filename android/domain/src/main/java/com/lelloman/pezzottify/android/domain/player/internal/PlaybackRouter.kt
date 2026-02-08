package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackModeManager
import com.lelloman.pezzottify.android.domain.player.PlaybackMode
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.player.VolumeState
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Routes playback operations to either the local PlayerImpl or the RemotePlaybackController
 * based on the current PlaybackMode.
 */
@OptIn(DelicateCoroutinesApi::class, ExperimentalCoroutinesApi::class)
@Singleton
class PlaybackRouter @Inject constructor(
    private val localPlayer: PlayerImpl,
    private val remoteController: RemotePlaybackController,
    private val playbackModeManager: PlaybackModeManager,
    private val playbackSessionHandler: PlaybackSessionHandler,
    loggerFactory: LoggerFactory,
) : PezzottifyPlayer {

    private val logger = loggerFactory.getLogger(PlaybackRouter::class)

    private val mode get() = playbackModeManager.mode.value
    private val isRemote get() = mode is PlaybackMode.Remote

    private val localOrRemote: ControlsAndStatePlayer
        get() = if (isRemote) remoteController else localPlayer

    // --- State flows that switch source based on mode ---

    override val isActive: StateFlow<Boolean> =
        MutableStateFlow(false).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.isActive
                        is PlaybackMode.Remote -> remoteController.isActive
                    }
                }.collect { flow.value = it }
            }
        }

    override val isPlaying: StateFlow<Boolean> =
        MutableStateFlow(false).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.isPlaying
                        is PlaybackMode.Remote -> remoteController.isPlaying
                    }
                }.collect { flow.value = it }
            }
        }

    override val volumeState: StateFlow<VolumeState> =
        MutableStateFlow(VolumeState(1f, false)).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.volumeState
                        is PlaybackMode.Remote -> remoteController.volumeState
                    }
                }.collect { flow.value = it }
            }
        }

    override val currentTrackIndex: StateFlow<Int?> =
        MutableStateFlow<Int?>(null).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.currentTrackIndex
                        is PlaybackMode.Remote -> remoteController.currentTrackIndex
                    }
                }.collect { flow.value = it }
            }
        }

    override val currentTrackPercent: StateFlow<Float?> =
        MutableStateFlow<Float?>(null).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.currentTrackPercent
                        is PlaybackMode.Remote -> remoteController.currentTrackPercent
                    }
                }.collect { flow.value = it }
            }
        }

    override val currentTrackProgressSec: StateFlow<Int?> =
        MutableStateFlow<Int?>(null).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.currentTrackProgressSec
                        is PlaybackMode.Remote -> remoteController.currentTrackProgressSec
                    }
                }.collect { flow.value = it }
            }
        }

    override val currentTrackDurationSeconds: StateFlow<Int?> =
        MutableStateFlow<Int?>(null).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.currentTrackDurationSeconds
                        is PlaybackMode.Remote -> remoteController.currentTrackDurationSeconds
                    }
                }.collect { flow.value = it }
            }
        }

    override val seekEvents: SharedFlow<ControlsAndStatePlayer.SeekEvent>
        get() = if (isRemote) remoteController.seekEvents else localPlayer.seekEvents

    override val playerError: StateFlow<ControlsAndStatePlayer.PlayerError?> =
        MutableStateFlow<ControlsAndStatePlayer.PlayerError?>(null).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.playerError
                        is PlaybackMode.Remote -> remoteController.playerError
                    }
                }.collect { flow.value = it }
            }
        }

    override val shuffleEnabled: StateFlow<Boolean> =
        MutableStateFlow(false).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.shuffleEnabled
                        is PlaybackMode.Remote -> remoteController.shuffleEnabled
                    }
                }.collect { flow.value = it }
            }
        }

    override val repeatMode: StateFlow<RepeatMode> =
        MutableStateFlow(RepeatMode.OFF).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.repeatMode
                        is PlaybackMode.Remote -> remoteController.repeatMode
                    }
                }.collect { flow.value = it }
            }
        }

    override val playbackPlaylist: StateFlow<PlaybackPlaylist?> =
        MutableStateFlow<PlaybackPlaylist?>(null).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.playbackPlaylist
                        is PlaybackMode.Remote -> flowOf(null) // Remote mode has no local playlist concept
                    }
                }.collect { flow.value = it }
            }
        }

    override val canGoToPreviousPlaylist: StateFlow<Boolean> =
        MutableStateFlow(false).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.canGoToPreviousPlaylist
                        is PlaybackMode.Remote -> flowOf(false)
                    }
                }.collect { flow.value = it }
            }
        }

    override val canGoToNextPlaylist: StateFlow<Boolean> =
        MutableStateFlow(false).also { flow ->
            GlobalScope.launch {
                playbackModeManager.mode.flatMapLatest { mode ->
                    when (mode) {
                        is PlaybackMode.Local -> localPlayer.canGoToNextPlaylist
                        is PlaybackMode.Remote -> flowOf(false)
                    }
                }.collect { flow.value = it }
            }
        }

    // --- ControlsAndStatePlayer control methods ---

    override fun togglePlayPause() = localOrRemote.togglePlayPause()
    override fun seekToPercentage(percentage: Float) = localOrRemote.seekToPercentage(percentage)
    override fun setIsPlaying(isPlaying: Boolean) = localOrRemote.setIsPlaying(isPlaying)
    override fun forward10Sec() = localOrRemote.forward10Sec()
    override fun rewind10Sec() = localOrRemote.rewind10Sec()
    override fun stop() = localOrRemote.stop()
    override fun setVolume(volume: Float) = localOrRemote.setVolume(volume)
    override fun setMuted(isMuted: Boolean) = localOrRemote.setMuted(isMuted)
    override fun loadTrackIndex(index: Int) = localOrRemote.loadTrackIndex(index)
    override fun skipToNextTrack() = localOrRemote.skipToNextTrack()
    override fun skipToPreviousTrack() = localOrRemote.skipToPreviousTrack()
    override fun toggleShuffle() = localOrRemote.toggleShuffle()
    override fun cycleRepeatMode() = localOrRemote.cycleRepeatMode()
    override fun retry() = localOrRemote.retry()

    // --- PezzottifyPlayer content methods ---

    override fun loadAlbum(albumId: String, startTrackId: String?) {
        if (isRemote) {
            val payload = mutableMapOf<String, Any?>("albumId" to albumId)
            if (startTrackId != null) payload["startTrackId"] = startTrackId
            sendRemoteCommand("loadAlbum", payload)
        } else {
            localPlayer.loadAlbum(albumId, startTrackId)
        }
    }

    override fun addAlbumToPlaylist(albumId: String) {
        if (isRemote) {
            sendRemoteCommand("addAlbumToQueue", mapOf("albumId" to albumId))
        } else {
            localPlayer.addAlbumToPlaylist(albumId)
        }
    }

    override fun loadUserPlaylist(userPlaylistId: String, startTrackId: String?) {
        if (isRemote) {
            val payload = mutableMapOf<String, Any?>("playlistId" to userPlaylistId)
            if (startTrackId != null) payload["startTrackId"] = startTrackId
            sendRemoteCommand("loadPlaylist", payload)
        } else {
            localPlayer.loadUserPlaylist(userPlaylistId, startTrackId)
        }
    }

    override fun addUserPlaylistToQueue(userPlaylistId: String) {
        if (isRemote) {
            sendRemoteCommand("addPlaylistToQueue", mapOf("playlistId" to userPlaylistId))
        } else {
            localPlayer.addUserPlaylistToQueue(userPlaylistId)
        }
    }

    override fun loadSingleTrack(trackId: String) {
        if (isRemote) {
            sendRemoteCommand("loadSingleTrack", mapOf("trackId" to trackId))
        } else {
            localPlayer.loadSingleTrack(trackId)
        }
    }

    override fun goToPreviousPlaylist() {
        if (!isRemote) localPlayer.goToPreviousPlaylist()
    }

    override fun goToNextPlaylist() {
        if (!isRemote) localPlayer.goToNextPlaylist()
    }

    override fun moveTrack(fromIndex: Int, toIndex: Int) {
        if (isRemote) {
            sendRemoteCommand("moveTrack", mapOf("fromIndex" to fromIndex, "toIndex" to toIndex))
        } else {
            localPlayer.moveTrack(fromIndex, toIndex)
        }
    }

    override fun addTracksToPlaylist(tracksIds: List<String>) {
        if (isRemote) {
            sendRemoteCommand("addTracksToQueue", mapOf("trackIds" to tracksIds))
        } else {
            localPlayer.addTracksToPlaylist(tracksIds)
        }
    }

    override fun removeTrackFromPlaylist(trackId: String) {
        if (isRemote) {
            sendRemoteCommand("removeTrack", mapOf("trackId" to trackId))
        } else {
            localPlayer.removeTrackFromPlaylist(trackId)
        }
    }

    override fun clearSession() {
        if (isRemote) {
            playbackModeManager.exitRemoteMode()
        }
        localPlayer.clearSession()
    }

    override suspend fun tryRestoreState(): Boolean {
        return if (isRemote) false else localPlayer.tryRestoreState()
    }

    override fun initialize() {
        localPlayer.initialize()
        remoteController.initialize()

        // Clear local playback session when entering remote mode so that
        // returning to local mode starts with an idle player instead of stale state.
        // Must use Main dispatcher because clearSession() calls MediaController
        // which requires the main thread.
        GlobalScope.launch(Dispatchers.Main) {
            playbackModeManager.mode.collect { mode ->
                if (mode is PlaybackMode.Remote) {
                    localPlayer.clearSession()
                }
            }
        }
    }

    private fun sendRemoteCommand(command: String, payload: Map<String, Any?>) {
        val deviceId = (mode as? PlaybackMode.Remote)?.deviceId ?: return
        playbackSessionHandler.sendCommand(command, payload, deviceId)
        logger.debug("Sent remote command '$command' to device $deviceId")
    }
}
