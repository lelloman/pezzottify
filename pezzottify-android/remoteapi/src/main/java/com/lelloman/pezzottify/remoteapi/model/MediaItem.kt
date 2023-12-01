package com.lelloman.pezzottify.remoteapi.model

data class Image(
    val id: String,
    val size: Long,
    val width: Int,
    val height: Int,
    val type: Type,
) {
    enum class Type {
        PNG, JPG
    }
}

enum class ArtistRole {
    Performer,
    Composer,
}

data class ArtistRelation(
    val id: String = "",
    val artistId: String,
    val role: ArtistRole = ArtistRole.Performer,
)

data class AudioTrack(
    val id: String = "",
    val size: Long,
    val name: String,
    val durationMs: Long,
    val sampleRate: Int,
    val bitRate: Long,
    val type: Type,
    val artists: List<ArtistRelation> = emptyList(),
) {
    enum class Type {
        MP3, FLAC;
    }
}