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
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn

@HiltViewModel(assistedFactory = ArtistScreenViewModel.Factory::class)
class ArtistScreenViewModel @AssistedInject constructor(
    val contentResolver: ContentResolver,
    @Assisted private val artistId: String,
) : ViewModel() {

    private val discographyFlow = flow {
        emit(null) // Start with null
        val discography = contentResolver.getArtistDiscography(artistId)
        emit(discography)
    }

    val state = combine(
        contentResolver.resolveArtist(artistId),
        discographyFlow
    ) { artistContent, discography ->
        when (artistContent) {
            is Content.Loading -> ArtistScreenState()
            is Content.Error -> ArtistScreenState(
                isError = true,
                isLoading = false,
            )
            is Content.Resolved -> ArtistScreenState(
                artist = artistContent.data,
                albums = discography?.albums ?: emptyList(),
                features = discography?.features ?: emptyList(),
                relatedArtists = artistContent.data.related,
                isError = false,
                isLoading = false,
            )
        }
    }.stateIn(viewModelScope, SharingStarted.Eagerly, ArtistScreenState())

    @AssistedFactory
    interface Factory {
        fun create(artistId: String): ArtistScreenViewModel
    }
}