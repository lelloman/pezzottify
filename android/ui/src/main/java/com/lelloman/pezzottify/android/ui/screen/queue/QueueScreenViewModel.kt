package com.lelloman.pezzottify.android.ui.screen.queue

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class QueueScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), QueueScreenActions {

    val state = interactor.getQueueState()
        .map { queueState ->
            if (queueState != null) {
                val tracks = queueState.tracks.map { track ->
                    QueueTrackItem(
                        trackId = track.trackId,
                        trackName = track.trackName,
                        albumId = track.albumId,
                        artists = track.artists,
                        durationSeconds = track.durationSeconds,
                        availability = track.availability,
                    )
                }
                QueueScreenState(
                    isLoading = false,
                    isError = false,
                    contextName = queueState.contextName,
                    contextType = queueState.contextType,
                    tracks = tracks,
                    currentTrackIndex = queueState.currentIndex,
                    canSaveAsPlaylist = queueState.canSaveAsPlaylist,
                )
            } else {
                QueueScreenState(isLoading = false, isError = true)
            }
        }
        .stateIn(viewModelScope, SharingStarted.Eagerly, QueueScreenState())

    override fun clickOnTrack(index: Int) {
        interactor.playTrackAtIndex(index)
    }

    override fun moveTrack(fromIndex: Int, toIndex: Int) {
        interactor.moveTrack(fromIndex, toIndex)
    }

    override fun removeTrack(trackId: String) {
        interactor.removeTrack(trackId)
    }

    override fun clickOnSaveAsPlaylist() {
        // TODO: Implement saving as user playlist
        // This will need additional UI for entering playlist name
    }

    override fun playTrackDirectly(trackId: String) {
        interactor.playTrackDirectly(trackId)
    }

    override fun addTrackToQueue(trackId: String) {
        interactor.addTrackToQueue(trackId)
    }

    override fun addTrackToPlaylist(trackId: String, targetPlaylistId: String) {
        viewModelScope.launch {
            interactor.addTrackToPlaylist(trackId, targetPlaylistId)
        }
    }

    override fun createPlaylist(name: String) {
        viewModelScope.launch {
            interactor.createPlaylist(name)
        }
    }

    override fun toggleTrackLike(trackId: String, currentlyLiked: Boolean) {
        interactor.toggleLike(trackId, currentlyLiked)
    }

    override fun getTrackLikeState(trackId: String): Flow<Boolean> =
        interactor.isLiked(trackId)

    override fun getUserPlaylists(): Flow<List<UiUserPlaylist>> =
        interactor.getUserPlaylists()

    interface Interactor {
        fun getQueueState(): Flow<QueueState?>
        fun playTrackAtIndex(index: Int)
        fun moveTrack(fromIndex: Int, toIndex: Int)
        fun removeTrack(trackId: String)

        // Track actions bottom sheet
        fun playTrackDirectly(trackId: String)
        fun addTrackToQueue(trackId: String)
        suspend fun addTrackToPlaylist(trackId: String, playlistId: String)
        suspend fun createPlaylist(name: String)
        fun toggleLike(trackId: String, currentlyLiked: Boolean)
        fun isLiked(trackId: String): Flow<Boolean>
        fun getUserPlaylists(): Flow<List<UiUserPlaylist>>

        data class QueueState(
            val tracks: List<QueueTrack>,
            val currentIndex: Int,
            val contextName: String,
            val contextType: QueueContextType,
            val canSaveAsPlaylist: Boolean,
        )

        data class QueueTrack(
            val trackId: String,
            val trackName: String,
            val albumId: String,
            val artists: List<ArtistInfo>,
            val durationSeconds: Int,
            val availability: com.lelloman.pezzottify.android.ui.content.TrackAvailability =
                com.lelloman.pezzottify.android.ui.content.TrackAvailability.Available,
        )
    }
}
