package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.toExternalAlbum
import com.lelloman.pezzottify.android.ui.toFullScreenImage
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private const val TAG = "ArtistScreenViewModel"

@HiltViewModel(assistedFactory = ArtistScreenViewModel.Factory::class)
class ArtistScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    @Assisted private val artistId: String,
    @Assisted private val navController: NavController,
) : ViewModel(), ArtistScreenActions {

    private var hasLoggedView = false
    private var hasLoadedExternalAlbums = false

    private val externalAlbumsState = MutableStateFlow<ExternalAlbumsState>(ExternalAlbumsState.Idle)

    val state = combine(
        contentResolver.resolveArtist(artistId),
        contentResolver.resolveArtistDiscography(artistId)
    ) { artistContent, discographyContent ->
        when (artistContent) {
            is Content.Loading -> ArtistScreenState()
            is Content.Error -> ArtistScreenState(
                isError = true,
                isLoading = false,
            )
            is Content.Resolved -> {
                val discography = (discographyContent as? Content.Resolved)?.data
                ArtistScreenState(
                    artist = artistContent.data,
                    albums = discography?.albums ?: emptyList(),
                    features = discography?.features ?: emptyList(),
                    relatedArtists = artistContent.data.related,
                    isError = false,
                    isLoading = false,
                )
            }
        }
    }.combine(interactor.isLiked(artistId)) { artistState, isLiked ->
        artistState.copy(isLiked = isLiked)
    }.combine(externalAlbumsState) { artistState, externalState ->
        when (externalState) {
            is ExternalAlbumsState.Idle -> artistState
            is ExternalAlbumsState.Loading -> artistState.copy(isLoadingExternalAlbums = true)
            is ExternalAlbumsState.Loaded -> artistState.copy(
                externalAlbums = externalState.albums,
                isLoadingExternalAlbums = false
            )
            is ExternalAlbumsState.Error -> artistState.copy(isLoadingExternalAlbums = false)
        }
    }.onEach { state ->
        if (!state.isLoading && !state.isError && !hasLoggedView) {
            hasLoggedView = true
            interactor.logViewedArtist(artistId)
        }
        // Load external albums once we have the artist loaded and external search is available
        if (!state.isLoading && !state.isError && !hasLoadedExternalAlbums) {
            hasLoadedExternalAlbums = true
            loadExternalAlbums()
        }
    }.stateIn(viewModelScope, SharingStarted.Eagerly, ArtistScreenState())

    private fun loadExternalAlbums() {
        viewModelScope.launch {
            val canShow = interactor.canShowExternalAlbums()
            Log.d(TAG, "loadExternalAlbums($artistId) canShowExternalAlbums=$canShow")
            if (!canShow) return@launch

            externalAlbumsState.value = ExternalAlbumsState.Loading
            val result = interactor.getExternalDiscography(artistId)
            Log.d(TAG, "loadExternalAlbums($artistId) result=$result")
            externalAlbumsState.value = result.fold(
                onSuccess = { albums ->
                    Log.d(TAG, "loadExternalAlbums($artistId) loaded ${albums.size} albums")
                    ExternalAlbumsState.Loaded(albums)
                },
                onFailure = { error ->
                    Log.e(TAG, "loadExternalAlbums($artistId) error", error)
                    ExternalAlbumsState.Error
                }
            )
        }
    }

    override fun clickOnLike() {
        interactor.toggleLike(artistId, state.value.isLiked)
    }

    override fun clickOnArtistImage(imageUrl: String?) {
        if (imageUrl != null) {
            navController.toFullScreenImage(imageUrl)
        }
    }

    override fun clickOnExternalAlbum(albumId: String) {
        navController.toExternalAlbum(albumId)
    }

    interface Interactor {
        fun logViewedArtist(artistId: String)
        fun isLiked(contentId: String): Flow<Boolean>
        fun toggleLike(contentId: String, currentlyLiked: Boolean)
        suspend fun canShowExternalAlbums(): Boolean
        suspend fun getExternalDiscography(artistId: String): Result<List<UiExternalAlbumItem>>
    }

    @AssistedFactory
    interface Factory {
        fun create(artistId: String, navController: NavController): ArtistScreenViewModel
    }

    private sealed class ExternalAlbumsState {
        data object Idle : ExternalAlbumsState()
        data object Loading : ExternalAlbumsState()
        data class Loaded(val albums: List<UiExternalAlbumItem>) : ExternalAlbumsState()
        data object Error : ExternalAlbumsState()
    }
}