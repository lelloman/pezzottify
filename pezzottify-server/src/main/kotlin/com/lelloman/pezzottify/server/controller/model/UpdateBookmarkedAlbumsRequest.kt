package com.lelloman.pezzottify.server.controller.model

data class UpdateBookmarkedAlbumsRequest(
    val albumsIdsToAdd: List<String> = emptyList(),
    val albumsIdsToRemove: List<String> = emptyList(),
)