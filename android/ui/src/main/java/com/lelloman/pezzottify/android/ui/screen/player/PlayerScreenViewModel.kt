package com.lelloman.pezzottify.android.ui.screen.player

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class PlayerScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), PlayerScreenActions {

    private val mutableState = MutableStateFlow(PlayerScreenState())
    val state = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            interactor.getPlaybackState().collect { playbackState ->
                mutableState.value = when (playbackState) {
                    null, is Interactor.PlaybackState.Idle -> {
                        PlayerScreenState(isLoading = true)
                    }
                    is Interactor.PlaybackState.Loaded -> {
                        PlayerScreenState(
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

    interface Interactor {
        fun getPlaybackState(): Flow<PlaybackState?>
        fun togglePlayPause()
        fun skipToNext()
        fun skipToPrevious()
        fun seekToPercent(percent: Float)
        fun setVolume(volume: Float)
        fun toggleMute()
        fun toggleShuffle()
        fun cycleRepeatMode()

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
            ) : PlaybackState
        }
    }
}
