package com.lelloman.pezzottify.android.ui.screen.player

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class PlayerScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), PlayerScreenActions {

    private val mutableState = MutableStateFlow(PlayerScreenState())
    val state = mutableState.asStateFlow()

    private val mutableToastEvents = MutableSharedFlow<String>(extraBufferCapacity = 1)
    val toastEvents: Flow<String> = mutableToastEvents.asSharedFlow()

    init {
        var lastErrorMessage: String? = null

        viewModelScope.launch {
            interactor.getRemoteDeviceName().collect { name ->
                mutableState.value = mutableState.value.copy(remoteDeviceName = name)
            }
        }

        viewModelScope.launch {
            interactor.getPlaybackState().collect { playbackState ->
                val currentState = mutableState.value
                mutableState.value = when (playbackState) {
                    null, is Interactor.PlaybackState.Idle -> {
                        currentState.copy(isLoading = true)
                    }
                    is Interactor.PlaybackState.Loaded -> {
                        // Check for new error to show toast
                        val error = playbackState.playerError
                        if (error != null && error.message != lastErrorMessage) {
                            lastErrorMessage = error.message
                            when {
                                error.isRecoverable -> {
                                    // Transient error - show retry message
                                    mutableToastEvents.emit("Playback failed: ${error.message}. Retrying...")
                                }
                                else -> {
                                    // Permanent error - show skip message
                                    mutableToastEvents.emit("Playback failed: ${error.message}. Skipping to next track.")
                                }
                            }
                        } else if (error == null) {
                            // Error cleared - reset tracking state
                            lastErrorMessage = null
                        }

                        currentState.copy(
                            isLoading = false,
                            trackId = playbackState.trackId,
                            trackName = playbackState.trackName,
                            albumId = playbackState.albumId,
                            albumName = playbackState.albumName,
                            albumImageUrl = playbackState.albumImageUrl,
                            artists = playbackState.artists,
                            isPlaying = playbackState.isPlaying,
                            trackProgressPercent = playbackState.trackPercent,
                            trackProgressSec = playbackState.trackProgressSec,
                            trackDurationSec = playbackState.trackDurationSec,
                            hasNextTrack = playbackState.hasNextTrack,
                            hasPreviousTrack = playbackState.hasPreviousTrack,
                            volume = playbackState.volume,
                            isMuted = playbackState.isMuted,
                            shuffleEnabled = playbackState.shuffleEnabled,
                            repeatMode = playbackState.repeatMode,
                            playerError = playbackState.playerError,
                        )
                    }
                }
            }
        }

    }

    override fun clickOnPlayPause() = interactor.togglePlayPause()

    override fun clickOnSkipNext() = interactor.skipToNext()

    override fun clickOnSkipPrevious() = interactor.skipToPrevious()

    override fun seekToPercent(percent: Float) = interactor.seekToPercent(percent)

    override fun setVolume(volume: Float) = interactor.setVolume(volume)

    override fun toggleMute() = interactor.toggleMute()

    override fun clickOnShuffle() = interactor.toggleShuffle()

    override fun clickOnRepeat() = interactor.cycleRepeatMode()

    override fun retry() = interactor.retry()

    override fun exitRemoteMode() = interactor.exitRemoteMode()

    interface Interactor {
        fun getPlaybackState(): Flow<PlaybackState?>
        fun getRemoteDeviceName(): Flow<String?>
        fun togglePlayPause()
        fun skipToNext()
        fun skipToPrevious()
        fun seekToPercent(percent: Float)
        fun setVolume(volume: Float)
        fun toggleMute()
        fun toggleShuffle()
        fun cycleRepeatMode()
        fun retry()
        fun exitRemoteMode()

        sealed interface PlaybackState {
            data object Idle : PlaybackState

            data class Loaded(
                val isPlaying: Boolean,
                val trackId: String,
                val trackName: String,
                val albumId: String,
                val albumName: String,
                val albumImageUrl: String?,
                val artists: List<ArtistInfo>,
                val trackPercent: Float,
                val trackProgressSec: Int,
                val trackDurationSec: Int,
                val hasNextTrack: Boolean,
                val hasPreviousTrack: Boolean,
                val volume: Float,
                val isMuted: Boolean,
                val shuffleEnabled: Boolean,
                val repeatMode: RepeatModeUi,
                val playerError: PlayerErrorUi?,
            ) : PlaybackState
        }
    }
}
