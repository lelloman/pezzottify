package com.lelloman.pezzottify.android.ui.screen.main

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.screen.player.PendingPlaybackUi
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class MainScreenViewModel @Inject constructor(
    private val interactor: Interactor,
    loggerFactory: LoggerFactory,
) : ViewModel(), MainScreenActions {

    private val logger by loggerFactory

    private val mutableState = MutableStateFlow(MainScreenState())
    val state = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            interactor.getRadioContinuationError().collect { hasError ->
                mutableState.value = mutableState.value.copy(radioContinuationError = hasError)
            }
        }
        viewModelScope.launch {
            interactor.getPlaybackState()
                .collect { playbackState ->
                    val newBottomPlayerState = when (playbackState) {
                        is Interactor.PlaybackState.Idle, null -> MainScreenState.BottomPlayer()
                        is Interactor.PlaybackState.Pending -> MainScreenState.BottomPlayer(
                            isVisible = true,
                            pending = playbackState.pending,
                        )
                        is Interactor.PlaybackState.Loading -> {
                            // Show bottom player with loading state while metadata is being fetched
                            MainScreenState.BottomPlayer(
                                isVisible = true,
                                isLoading = true,
                            )
                        }
                        is Interactor.PlaybackState.Loaded -> {
                            MainScreenState.BottomPlayer(
                                isVisible = true,
                                isLoading = false,
                                trackId = playbackState.trackId,
                                trackName = playbackState.trackName,
                                albumName = playbackState.albumName,
                                albumImageUrl = playbackState.albumImageUrl,
                                artists = playbackState.artists,
                                isPlaying = playbackState.isPlaying,
                                trackPercent = playbackState.trackPercent,
                                nextTrackName = playbackState.nextTrackName,
                                nextTrackArtists = playbackState.nextTrackArtists,
                                previousTrackName = playbackState.previousTrackName,
                                previousTrackArtists = playbackState.previousTrackArtists,
                            )
                        }
                    }
                    mutableState.value =
                        mutableState.value.copy(bottomPlayer = newBottomPlayerState)
                }
        }

        viewModelScope.launch {
            interactor.getNotificationUnreadCount().collect { count ->
                val oldState = mutableState.value
                mutableState.value = oldState.copy(notificationUnreadCount = count)
            }
        }

        viewModelScope.launch {
            interactor.getRemoteDeviceName().collect { deviceName ->
                mutableState.value = mutableState.value.copy(remoteDeviceName = deviceName)
            }
        }

        viewModelScope.launch {
            interactor.getHasOtherDeviceConnected().collect { hasOtherDeviceConnected ->
                mutableState.value = mutableState.value.copy(hasOtherDeviceConnected = hasOtherDeviceConnected)
            }
        }
    }

    override fun clickOnPlayPause() = interactor.clickOnPlayPause()
    override fun cancelPendingPlayback() = interactor.cancelPendingPlayback()
    override fun retryPendingPlayback() = interactor.retryPendingPlayback()
    override fun retryRadioContinuation() = interactor.retryRadioContinuation()
    override fun dismissRadioContinuationError() = interactor.dismissRadioContinuationError()

    override fun clickOnSkipToNext() = interactor.clickOnSkipToNext()

    override fun clickOnSkipToPrevious() = interactor.clickOnSkipToPrevious()

    interface Interactor {
        fun getRadioContinuationError(): Flow<Boolean> = kotlinx.coroutines.flow.flowOf(false)
        fun cancelPendingPlayback() {}
        fun retryPendingPlayback() {}
        fun retryRadioContinuation() {}
        fun dismissRadioContinuationError() {}

        fun getPlaybackState(): Flow<PlaybackState?>

        fun getNotificationUnreadCount(): Flow<Int>

        fun getRemoteDeviceName(): Flow<String?>

        fun getHasOtherDeviceConnected(): Flow<Boolean>

        fun clickOnPlayPause()

        fun clickOnSkipToNext()

        fun clickOnSkipToPrevious()

        sealed interface PlaybackState {
            data object Idle : PlaybackState

            data object Loading : PlaybackState

            /** Playback requested but not playable yet; wins over Idle/Loading/Loaded. */
            data class Pending(val pending: PendingPlaybackUi) : PlaybackState

            data class Loaded(
                val isPlaying: Boolean,
                val trackId: String,
                val trackName: String,
                val albumName: String,
                val albumImageUrl: String?,
                val artists: List<ArtistInfo>,
                val trackPercent: Float,
                val nextTrackName: String?,
                val nextTrackArtists: List<ArtistInfo>,
                val previousTrackName: String?,
                val previousTrackArtists: List<ArtistInfo>,
            ) : PlaybackState
        }
    }
}
