package com.lelloman.pezzottify.remoteapi.model

interface Artist {
    val id: String
    val displayName: String
    val image: Image?
}

data class IndividualArtist(
    override val id: String = "",
    override val displayName: String,
    override val image: Image? = null,
    val firstName: String? = null,
    val lastName: String? = null,
) : Artist

data class BandArtist(
    override val id: String = "",
    override val displayName: String,
    override val image: Image? = null,
    val members: List<Artist>,
) : Artist