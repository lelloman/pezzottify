package com.lelloman.pezzottify.android.ui.screen.main.content.track

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import androidx.lifecycle.viewModelScope
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.toFullScreenImage
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel(assistedFactory = TrackScreenViewModel.Factory::class)
class TrackScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    @Assisted private val trackId: String,
    @Assisted private val navController: NavController,
) : ViewModel(), TrackScreenActions {

    private var hasLoggedView = false

    val state = contentResolver.resolveTrack(trackId)
        .flatMapLatest { trackContent ->
            when (trackContent) {
                is Content.Loading -> flowOf(TrackScreenState())
                is Content.Error -> flowOf(
                    TrackScreenState(
                        isError = true,
                        isLoading = false,
                    )
                )
                is Content.Resolved -> {
                    contentResolver.resolveAlbum(trackContent.data.albumId)
                        .map { albumContent ->
                            TrackScreenState(
                                track = trackContent.data,
                                album = (albumContent as? Content.Resolved)?.data,
                                isError = false,
                                isLoading = false,
                            )
                        }
                }
            }
        }
        .combine(interactor.getCurrentPlayingTrackId()) { trackState, currentTrackId ->
            trackState.copy(currentPlayingTrackId = currentTrackId)
        }
        .combine(interactor.isLiked(trackId)) { trackState, isLiked ->
            trackState.copy(isLiked = isLiked)
        }
        .onEach { state ->
            if (!state.isLoading && !state.isError && !hasLoggedView) {
                hasLoggedView = true
                interactor.logViewedTrack(trackId)
            }
        }
        .stateIn(viewModelScope, SharingStarted.Eagerly, TrackScreenState())

    override fun clickOnPlayTrack() {
        state.value.track?.let { track ->
            interactor.playTrack(track.albumId, trackId)
        }
    }

    override fun clickOnLike() {
        interactor.toggleLike(trackId, state.value.isLiked)
    }

    fun clickOnAlbum(albumId: String) {
        navController.navigate(com.lelloman.pezzottify.android.ui.Screen.Main.Album(albumId))
    }

    fun clickOnArtist(artistId: String) {
        navController.navigate(com.lelloman.pezzottify.android.ui.Screen.Main.Artist(artistId))
    }

    fun clickOnAlbumImage(imageUrl: String?) {
        navController.toFullScreenImage(imageUrl)
    }

    interface Interactor {
        fun playTrack(albumId: String, trackId: String)
        fun logViewedTrack(trackId: String)
        fun getCurrentPlayingTrackId(): Flow<String?>
        fun isLiked(contentId: String): Flow<Boolean>
        fun toggleLike(contentId: String, currentlyLiked: Boolean)
    }

    @AssistedFactory
    interface Factory {
        fun create(trackId: String, navController: NavController): TrackScreenViewModel
    }
}
