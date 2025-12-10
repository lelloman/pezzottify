package com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

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
        .combine(interactor.getUserPlaylists()) { state, playlists ->
            state.copy(userPlaylists = playlists)
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

    override fun playTrackDirectly(trackId: String) {
        // Play only this single track (not the playlist context)
        interactor.playTrackDirectly(trackId)
    }

    override fun addTrackToQueue(trackId: String) {
        interactor.addTrackToQueue(trackId)
    }

    override fun addPlaylistToQueue() {
        interactor.addPlaylistToQueue(playlistId)
    }

    override fun addTrackToPlaylist(trackId: String, targetPlaylistId: String) {
        viewModelScope.launch {
            interactor.addTrackToPlaylist(trackId, targetPlaylistId)
        }
    }

    override fun removeTrackFromPlaylist(trackId: String) {
        viewModelScope.launch {
            interactor.removeTrackFromPlaylist(playlistId, trackId)
        }
    }

    override fun createPlaylist(name: String) {
        viewModelScope.launch {
            interactor.createPlaylist(name)
        }
    }

    interface Interactor {
        fun getPlaylist(playlistId: String): Flow<UiUserPlaylistDetails?>
        fun playPlaylist(playlistId: String)
        fun playTrack(playlistId: String, trackId: String)
        fun logViewedPlaylist(playlistId: String)
        fun getCurrentPlayingTrackId(): Flow<String?>
        fun getUserPlaylists(): Flow<List<UiUserPlaylist>>

        // Methods for bottom sheet actions
        fun playTrackDirectly(trackId: String)
        fun addTrackToQueue(trackId: String)
        fun addPlaylistToQueue(playlistId: String)
        suspend fun addTrackToPlaylist(trackId: String, playlistId: String)
        suspend fun removeTrackFromPlaylist(playlistId: String, trackId: String)
        suspend fun createPlaylist(name: String)
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

    // New actions for bottom sheets
    fun playTrackDirectly(trackId: String)
    fun addTrackToQueue(trackId: String)
    fun addPlaylistToQueue()
    fun addTrackToPlaylist(trackId: String, targetPlaylistId: String)
    fun removeTrackFromPlaylist(trackId: String)
    fun createPlaylist(name: String)
}
