package com.lelloman.pezzottify.android.ui.model

import kotlinx.serialization.json.JsonObject

sealed interface PlaybackPlaylistContext {
    data class Album(val albumId: String) : PlaybackPlaylistContext
    data class UserPlaylist(val userPlaylistId: String, val isEdited: Boolean) : PlaybackPlaylistContext
    data object UserMix : PlaybackPlaylistContext
    data class Radio(
        val source: String,
        val seedEntityType: String,
        val seedEntityId: String,
        val seedLabel: String,
        val count: Int,
        val settings: JsonObject? = null,
        val isEdited: Boolean = false,
    ) : PlaybackPlaylistContext
}

data class PlaybackPlaylist(
    val context: PlaybackPlaylistContext,
    val tracksIds: List<String>,
)
