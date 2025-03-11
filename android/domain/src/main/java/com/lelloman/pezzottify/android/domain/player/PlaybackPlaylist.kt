package com.lelloman.pezzottify.android.domain.player

import androidx.annotation.FloatRange


sealed interface PlaybackPlaylistContext {

    data class Album(val albumId: String) : PlaybackPlaylistContext

    data class UserPlaylist(val userPlaylistId: String, val isEdited: Boolean) :
        PlaybackPlaylistContext

    data object UserMix : PlaybackPlaylistContext
}

data class PlaybackPlaylist(
    val context: PlaybackPlaylistContext,
    val tracksIds: List<String>,
    val currentTrackIndex: Int?,
    @FloatRange(0.0, 1.0) val currentTrackPercent: Float,
    val progressSec: Int?,
)
