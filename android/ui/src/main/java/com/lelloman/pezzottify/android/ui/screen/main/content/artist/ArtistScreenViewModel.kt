package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn

@HiltViewModel(assistedFactory = ArtistScreenViewModel.Factory::class)
class ArtistScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    @Assisted private val artistId: String,
) : ViewModel() {

    private var hasLoggedView = false

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
    }.onEach { state ->
        if (!state.isLoading && !state.isError && !hasLoggedView) {
            hasLoggedView = true
            interactor.logViewedArtist(artistId)
        }
    }.stateIn(viewModelScope, SharingStarted.Eagerly, ArtistScreenState())

    interface Interactor {
        fun logViewedArtist(artistId: String)
    }

    @AssistedFactory
    interface Factory {
        fun create(artistId: String): ArtistScreenViewModel
    }
}