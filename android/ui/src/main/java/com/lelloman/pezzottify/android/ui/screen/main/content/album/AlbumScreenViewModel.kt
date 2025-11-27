package com.lelloman.pezzottify.android.ui.screen.main.content.album

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.toFullScreenImage
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn

@HiltViewModel(assistedFactory = AlbumScreenViewModel.Factory::class)
class AlbumScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    private val contentResolver: ContentResolver,
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
        interactor.playTrack(albumId, trackId)
    }

    override fun clickOnAlbumImage(imageUrls: List<String>) {
        navController.toFullScreenImage(imageUrls)
    }

    interface Interactor {
        fun playAlbum(albumId: String)
        fun playTrack(albumId: String, trackId: String)
        fun logViewedAlbum(albumId: String)
        fun getCurrentPlayingTrackId(): kotlinx.coroutines.flow.Flow<String?>
    }

    @AssistedFactory
    interface Factory {
        fun create(albumId: String, navController: NavController): AlbumScreenViewModel
    }
}