package com.lelloman.pezzottify.android.ui.screen.queue

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

@HiltViewModel
class QueueScreenViewModel @Inject constructor(
    private val interactor: Interactor,
    private val contentResolver: ContentResolver,
) : ViewModel(), QueueScreenActions {

    val state = interactor.getPlaybackPlaylist()
        .combine(interactor.getCurrentTrackIndex()) { playlist, currentIndex ->
            if (playlist != null) {
                val tracks = playlist.tracksIds.map { trackId ->
                    contentResolver.resolveTrack(trackId)
                }
                val playlistContext = playlist.context
                val (contextType, contextName, canSave) = when (playlistContext) {
                    is PlaybackPlaylistContext.Album -> Triple(
                        QueueContextType.Album,
                        playlistContext.albumId,
                        false
                    )
                    is PlaybackPlaylistContext.UserPlaylist -> Triple(
                        QueueContextType.UserPlaylist,
                        playlistContext.userPlaylistId,
                        playlistContext.isEdited
                    )
                    is PlaybackPlaylistContext.UserMix -> Triple(
                        QueueContextType.UserMix,
                        "Your Mix",
                        true
                    )
                }
                QueueScreenState(
                    isLoading = false,
                    isError = false,
                    contextName = contextName,
                    contextType = contextType,
                    tracks = tracks,
                    currentTrackIndex = currentIndex,
                    canSaveAsPlaylist = canSave,
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

    interface Interactor {
        fun getPlaybackPlaylist(): Flow<PlaybackPlaylist?>
        fun getCurrentTrackIndex(): Flow<Int?>
        fun playTrackAtIndex(index: Int)
        fun moveTrack(fromIndex: Int, toIndex: Int)
        fun removeTrack(trackId: String)
    }
}
