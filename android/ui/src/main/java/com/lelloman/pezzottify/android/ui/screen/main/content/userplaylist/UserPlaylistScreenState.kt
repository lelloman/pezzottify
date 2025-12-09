package com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist

import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.Track
import kotlinx.coroutines.flow.Flow

data class UserPlaylistScreenState(
    val playlistId: String = "",
    val playlistName: String = "",
    val tracks: List<Flow<Content<Track>>>? = null,
    val isLoading: Boolean = true,
    val isError: Boolean = false,
    val currentPlayingTrackId: String? = null,
    val isAddToQueueMode: Boolean = false,
)
