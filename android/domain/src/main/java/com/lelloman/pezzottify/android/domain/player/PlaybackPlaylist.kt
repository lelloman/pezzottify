package com.lelloman.pezzottify.android.domain.player


sealed interface PlaybackPlaylistContext {

    data class Album(val albumId: String) : PlaybackPlaylistContext

    data class UserPlaylist(val userPlaylistId: String, val isEdited: Boolean) :
        PlaybackPlaylistContext

    data object UserMix : PlaybackPlaylistContext
}

data class PlaybackPlaylist(
    val context: PlaybackPlaylistContext,
    val tracksIds: List<String>,
)
