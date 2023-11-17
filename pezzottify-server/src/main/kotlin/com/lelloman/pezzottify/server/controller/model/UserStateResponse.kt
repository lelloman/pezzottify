package com.lelloman.pezzottify.server.controller.model

import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.model.UserPlayList

data class UserStateResponse(
    val bookmarkedAlbums: Set<String>,
    val playlists: List<UserPlayList>,
)