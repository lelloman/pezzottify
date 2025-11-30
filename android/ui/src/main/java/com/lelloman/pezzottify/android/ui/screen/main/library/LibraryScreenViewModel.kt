package com.lelloman.pezzottify.android.ui.screen.main.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

@HiltViewModel
class LibraryScreenViewModel @Inject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
) : ViewModel() {

    val state = interactor.getLikedContent()
        .map { likedItems ->
            val albums = likedItems
                .filter { it.contentType == LikedContent.ContentType.Album && it.isLiked }
                .map { it.contentId }
            val artists = likedItems
                .filter { it.contentType == LikedContent.ContentType.Artist && it.isLiked }
                .map { it.contentId }
            LibraryScreenState(
                likedAlbumIds = albums,
                likedArtistIds = artists,
                isLoading = false,
            )
        }
        .stateIn(viewModelScope, SharingStarted.Eagerly, LibraryScreenState())

    interface Interactor {
        fun getLikedContent(): Flow<List<LikedContent>>
    }
}
