package com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn

@HiltViewModel(assistedFactory = UserPlaylistScreenViewModel.Factory::class)
class UserPlaylistScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    @Assisted private val playlistId: String,
) : ViewModel(), UserPlaylistScreenActions {

    private var hasLoggedView = false

    val state = interactor.getPlaylist(playlistId)
        .combine(interactor.getCurrentPlayingTrackId()) { playlist, currentTrackId ->
            if (playlist != null) {
                val tracks = playlist.trackIds.map { trackId -> contentResolver.resolveTrack(trackId) }
                UserPlaylistScreenState(
                    playlistId = playlist.id,
                    playlistName = playlist.name,
                    tracks = tracks,
                    isLoading = false,
                    isError = false,
                    currentPlayingTrackId = currentTrackId,
                )
            } else {
                UserPlaylistScreenState(
                    isLoading = false,
                    isError = true,
                )
            }
        }
        .combine(interactor.getIsAddToQueueMode()) { state, isAddToQueue ->
            state.copy(isAddToQueueMode = isAddToQueue)
        }
        .onEach { state ->
            if (!state.isLoading && !state.isError && !hasLoggedView) {
                hasLoggedView = true
                interactor.logViewedPlaylist(playlistId)
            }
        }
        .stateIn(viewModelScope, SharingStarted.Eagerly, UserPlaylistScreenState())

    override fun clickOnPlayPlaylist() {
        interactor.playPlaylist(playlistId)
    }

    override fun clickOnTrack(trackId: String) {
        interactor.playTrack(playlistId, trackId)
    }

    interface Interactor {
        fun getPlaylist(playlistId: String): Flow<UiUserPlaylistDetails?>
        fun playPlaylist(playlistId: String)
        fun playTrack(playlistId: String, trackId: String)
        fun logViewedPlaylist(playlistId: String)
        fun getCurrentPlayingTrackId(): Flow<String?>
        fun getIsAddToQueueMode(): Flow<Boolean>
    }

    @AssistedFactory
    interface Factory {
        fun create(playlistId: String): UserPlaylistScreenViewModel
    }
}

data class UiUserPlaylistDetails(
    val id: String,
    val name: String,
    val trackIds: List<String>,
)

interface UserPlaylistScreenActions {
    fun clickOnPlayPlaylist()
    fun clickOnTrack(trackId: String)
}
