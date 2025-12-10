package com.lelloman.pezzottify.android.ui.screen.main.content.album

import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import kotlinx.coroutines.flow.Flow

data class AlbumScreenState(
    val album: Album? = null,
    val tracks: List<Flow<Content<Track>>>? = null,
    val isLoading: Boolean = true,
    val isError: Boolean = false,
    val currentPlayingTrackId: String? = null,
    val isLiked: Boolean = false,
    val userPlaylists: List<UiUserPlaylist> = emptyList(),
)