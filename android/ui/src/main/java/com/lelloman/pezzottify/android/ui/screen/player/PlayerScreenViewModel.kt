package com.lelloman.pezzottify.android.ui.screen.player

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.launch
import javax.inject.Inject

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class PlayerScreenViewModel @Inject constructor(
    private val interactor: Interactor,
    private val contentResolver: ContentResolver,
) : ViewModel(), PlayerScreenActions {

    private val mutableState = MutableStateFlow(PlayerScreenState())
    val state = mutableState.asStateFlow()

    private val currentTrackId = MutableStateFlow<String?>(null)

    init {
        viewModelScope.launch {
            interactor.getPlaybackState().collect { playbackState ->
                when (playbackState) {
                    null, is Interactor.PlaybackState.Idle -> {
                        mutableState.value = PlayerScreenState(isLoading = true)
                    }
                    is Interactor.PlaybackState.Loaded -> {
                        currentTrackId.value = playbackState.trackId
                        mutableState.value = mutableState.value.copy(
                            isLoading = false,
                            isPlaying = playbackState.isPlaying,
                            trackProgressPercent = playbackState.trackPercent,
                            trackProgressSec = playbackState.trackProgressSec,
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

        viewModelScope.launch {
            currentTrackId.filterNotNull()
                .flatMapLatest { trackId ->
                    contentResolver.resolveTrack(trackId)
                        .filterIsInstance<Content.Resolved<Track>>()
                        .take(1)
                }
                .collect { resolved ->
                    mutableState.value = mutableState.value.copy(
                        trackId = resolved.data.id,
                        trackName = resolved.data.name,
                        albumId = resolved.data.albumId,
                        artists = resolved.data.artists,
                        trackDurationSec = resolved.data.durationSeconds,
                    )
                }
        }

        viewModelScope.launch {
            currentTrackId.filterNotNull()
                .flatMapLatest { trackId ->
                    contentResolver.resolveTrack(trackId)
                        .filterIsInstance<Content.Resolved<Track>>()
                        .take(1)
                }
                .flatMapLatest { resolved ->
                    contentResolver.resolveAlbum(resolved.data.albumId)
                        .filterIsInstance<Content.Resolved<Album>>()
                        .take(1)
                }
                .collect { resolved ->
                    mutableState.value = mutableState.value.copy(
                        albumName = resolved.data.name,
                        albumImageUrl = resolved.data.imageUrl,
                    )
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
                val trackPercent: Float,
                val trackProgressSec: Int,
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
