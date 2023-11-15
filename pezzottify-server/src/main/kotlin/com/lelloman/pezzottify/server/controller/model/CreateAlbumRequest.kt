package com.lelloman.pezzottify.server.controller.model

data class CreateAlbumRequest(
    val name: String,
    val artistsIds: List<String>,
    val audioTracksNames: List<String>,
)