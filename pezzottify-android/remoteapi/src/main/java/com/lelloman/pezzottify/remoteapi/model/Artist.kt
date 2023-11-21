package com.lelloman.pezzottify.remoteapi.model

import com.google.gson.JsonDeserializationContext
import com.google.gson.JsonDeserializer
import com.google.gson.JsonElement
import java.lang.reflect.Type

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