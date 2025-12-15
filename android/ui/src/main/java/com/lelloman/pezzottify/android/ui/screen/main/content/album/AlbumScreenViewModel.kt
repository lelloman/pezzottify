package com.lelloman.pezzottify.android.ui.screen.main.content.album

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import com.lelloman.pezzottify.android.ui.toFullScreenImage
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

@HiltViewModel(assistedFactory = AlbumScreenViewModel.Factory::class)
class AlbumScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    @Assisted private val albumId: String,
    @Assisted private val navController: NavController,
) : ViewModel(), AlbumScreenActions {

    private var hasLoggedView = false

    val state = contentResolver.resolveAlbum(albumId)
        .map {
            when (it) {
                is Content.Loading -> AlbumScreenState()
                is Content.Error -> AlbumScreenState(
                    isError = true,
                    isLoading = false,
                )

                is Content.Resolved -> {
                    val trackIds = it.data.discs.flatMap { disc -> disc.tracksIds }
                    val tracks = trackIds.map { trackId -> contentResolver.resolveTrack(trackId) }
                    AlbumScreenState(
                        album = it.data,
                        tracks = tracks,
                        isError = false,
                        isLoading = false,
                    )
                }
            }
        }
        .combine(interactor.getCurrentPlayingTrackId()) { albumState, currentTrackId ->
            albumState.copy(currentPlayingTrackId = currentTrackId)
        }
        .combine(interactor.isLiked(albumId)) { albumState, isLiked ->
            albumState.copy(isLiked = isLiked)
        }
        .combine(interactor.getUserPlaylists()) { albumState, playlists ->
            albumState.copy(userPlaylists = playlists)
        }
        .onEach { state ->
            if (!state.isLoading && !state.isError && !hasLoggedView) {
                hasLoggedView = true
                interactor.logViewedAlbum(albumId)
            }
        }
        .stateIn(viewModelScope, SharingStarted.Eagerly, AlbumScreenState())

    override fun clickOnPlayAlbum(albumId: String) {
        interactor.playAlbum(albumId)
    }

    override fun clickOnTrack(trackId: String) {
        // When clicking a track directly, play it (replaces current queue with album starting from track)
        interactor.playTrack(albumId, trackId)
    }

    override fun clickOnAlbumImage(imageUrl: String?) {
        if (imageUrl != null) {
            navController.toFullScreenImage(imageUrl)
        }
    }

    override fun clickOnLike() {
        interactor.toggleLike(albumId, state.value.isLiked)
    }

    override fun playTrackDirectly(trackId: String) {
        // Play only this single track (not the album context)
        interactor.playTrackDirectly(trackId)
    }

    override fun addTrackToQueue(trackId: String) {
        interactor.addTrackToQueue(trackId)
    }

    override fun addAlbumToQueue(albumId: String) {
        interactor.addAlbumToQueue(albumId)
    }

    override fun addTrackToPlaylist(trackId: String, playlistId: String) {
        viewModelScope.launch {
            interactor.addTrackToPlaylist(trackId, playlistId)
        }
    }

    override fun addAlbumToPlaylist(albumId: String, playlistId: String) {
        viewModelScope.launch {
            interactor.addAlbumToPlaylist(albumId, playlistId)
        }
    }

    override fun createPlaylist(name: String) {
        viewModelScope.launch {
            interactor.createPlaylist(name)
        }
    }

    override fun toggleTrackLike(trackId: String, currentlyLiked: Boolean) {
        interactor.toggleTrackLike(trackId, currentlyLiked)
    }

    override fun getTrackLikeState(trackId: String): Flow<Boolean> = interactor.isLiked(trackId)

    interface Interactor {
        fun playAlbum(albumId: String)
        fun playTrack(albumId: String, trackId: String)
        fun logViewedAlbum(albumId: String)
        fun getCurrentPlayingTrackId(): Flow<String?>
        fun isLiked(contentId: String): Flow<Boolean>
        fun toggleLike(contentId: String, currentlyLiked: Boolean)
        fun toggleTrackLike(trackId: String, currentlyLiked: Boolean)
        fun getUserPlaylists(): Flow<List<UiUserPlaylist>>

        // Methods for bottom sheet actions
        fun playTrackDirectly(trackId: String)
        fun addTrackToQueue(trackId: String)
        fun addAlbumToQueue(albumId: String)
        suspend fun addTrackToPlaylist(trackId: String, playlistId: String)
        suspend fun addAlbumToPlaylist(albumId: String, playlistId: String)
        suspend fun createPlaylist(name: String)
    }

    @AssistedFactory
    interface Factory {
        fun create(albumId: String, navController: NavController): AlbumScreenViewModel
    }
}
