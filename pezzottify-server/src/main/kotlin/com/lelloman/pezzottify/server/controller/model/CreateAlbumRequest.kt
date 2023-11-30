package com.lelloman.pezzottify.server.controller.model

import com.lelloman.pezzottify.server.model.ArtistRelation

data class CreateAlbumRequest(
    val name: String,
    val artistsIds: List<String>,
    val audioTracksDefs: List<AudioTrackDef>,
) {
    data class AudioTrackDef(
        val name: String,
        val artists: List<ArtistRelation> = emptyList(),
    )
}