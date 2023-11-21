package com.lelloman.pezzottify.remoteapi.model

data class UserStateResponse(
    val bookmarkedAlbums: Set<String>,
    val playlists: List<UserPlayList>,
)