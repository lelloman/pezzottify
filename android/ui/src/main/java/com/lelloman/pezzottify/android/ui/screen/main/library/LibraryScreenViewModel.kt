package com.lelloman.pezzottify.android.ui.screen.main.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.model.LikedContent
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

@HiltViewModel
class LibraryScreenViewModel @Inject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
) : ViewModel() {

    val state = combine(
        interactor.getLikedContent(),
        interactor.getPlaylists(),
    ) { likedItems, playlists ->
        val albums = likedItems
            .filter { it.contentType == LikedContent.ContentType.Album && it.isLiked }
            .map { it.contentId }
        val artists = likedItems
            .filter { it.contentType == LikedContent.ContentType.Artist && it.isLiked }
            .map { it.contentId }
        val tracks = likedItems
            .filter { it.contentType == LikedContent.ContentType.Track && it.isLiked }
            .map { it.contentId }
        LibraryScreenState(
            likedAlbumIds = albums,
            likedArtistIds = artists,
            likedTrackIds = tracks,
            playlists = playlists,
            isLoading = false,
        )
    }.stateIn(viewModelScope, SharingStarted.Eagerly, LibraryScreenState())

    interface Interactor {
        fun getLikedContent(): Flow<List<LikedContent>>
        fun getPlaylists(): Flow<List<UiUserPlaylist>>
    }
}
