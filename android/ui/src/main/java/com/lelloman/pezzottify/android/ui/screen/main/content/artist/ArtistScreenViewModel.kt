package com.lelloman.pezzottify.android.ui.screen.main.content.artist

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
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

@HiltViewModel(assistedFactory = ArtistScreenViewModel.Factory::class)
class ArtistScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    @Assisted private val artistId: String,
    @Assisted private val navController: NavController,
) : ViewModel(), ArtistScreenActions {

    private var hasLoggedView = false

    init {
        viewModelScope.launch {
            // Clear error states to force retry of previously failed items
            interactor.retryErroredItems(listOf(artistId))
            // Fetch first page of albums and appears-on albums
            interactor.fetchFirstDiscographyPage(artistId)
            interactor.fetchFirstAppearsOnPage(artistId)
        }
    }

    val state = combine(
        contentResolver.resolveArtist(artistId),
        interactor.observeDiscographyState(artistId),
        interactor.isLiked(artistId),
        interactor.observeAppearsOnState(artistId)
    ) { artistContent, discographyState, isLiked, appearsOnState ->
        when (artistContent) {
            is Content.Loading -> ArtistScreenState()
            is Content.Error -> ArtistScreenState(
                isError = true,
                isLoading = false,
            )
            is Content.Resolved -> {
                ArtistScreenState(
                    artist = artistContent.data,
                    albums = discographyState.albumIds,
                    features = appearsOnState.albumIds,
                    relatedArtists = artistContent.data.related,
                    isError = false,
                    isLoading = false,
                    isLiked = isLiked,
                    hasMoreAlbums = discographyState.hasMore,
                    isLoadingMoreAlbums = discographyState.isLoading,
                    hasMoreFeatures = appearsOnState.hasMore,
                    isLoadingMoreFeatures = appearsOnState.isLoading,
                )
            }
        }
    }.onEach { state ->
        if (!state.isLoading && !state.isError && !hasLoggedView) {
            hasLoggedView = true
            interactor.logViewedArtist(artistId)
        }
    }.stateIn(viewModelScope, SharingStarted.Eagerly, ArtistScreenState())

    override fun clickOnLike() {
        interactor.toggleLike(artistId, state.value.isLiked)
    }

    override fun clickOnArtistImage(imageUrl: String?) {
        if (imageUrl != null) {
            navController.toFullScreenImage(imageUrl)
        }
    }

    override fun loadMoreAlbums() {
        viewModelScope.launch {
            interactor.fetchMoreDiscography(artistId)
        }
    }

    override fun loadMoreFeatures() {
        viewModelScope.launch {
            interactor.fetchMoreAppearsOn(artistId)
        }
    }

    /**
     * UI-layer representation of discography state.
     */
    data class DiscographyUiState(
        val albumIds: List<String>,
        val hasMore: Boolean,
        val isLoading: Boolean,
    )

    interface Interactor {
        fun logViewedArtist(artistId: String)
        fun isLiked(contentId: String): Flow<Boolean>
        fun toggleLike(contentId: String, currentlyLiked: Boolean)
        fun observeDiscographyState(artistId: String): Flow<DiscographyUiState>
        suspend fun fetchFirstDiscographyPage(artistId: String)
        suspend fun fetchMoreDiscography(artistId: String)
        fun observeAppearsOnState(artistId: String): Flow<DiscographyUiState>
        suspend fun fetchFirstAppearsOnPage(artistId: String)
        suspend fun fetchMoreAppearsOn(artistId: String)
        suspend fun retryErroredItems(itemIds: List<String>)
    }

    @AssistedFactory
    interface Factory {
        fun create(artistId: String, navController: NavController): ArtistScreenViewModel
    }
}
