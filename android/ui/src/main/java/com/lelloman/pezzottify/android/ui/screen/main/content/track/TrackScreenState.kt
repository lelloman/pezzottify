package com.lelloman.pezzottify.android.ui.screen.main.content.track

import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Track

data class TrackScreenState(
    val track: Track? = null,
    val album: Album? = null,
    val isLoading: Boolean = true,
    val isError: Boolean = false,
    val currentPlayingTrackId: String? = null,
    val isLiked: Boolean = false,
)
