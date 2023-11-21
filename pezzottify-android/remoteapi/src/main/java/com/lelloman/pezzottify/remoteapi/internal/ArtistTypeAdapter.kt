package com.lelloman.pezzottify.remoteapi.internal

import com.google.gson.JsonDeserializationContext
import com.google.gson.JsonDeserializer
import com.google.gson.JsonElement
import com.lelloman.pezzottify.remoteapi.model.Artist
import com.lelloman.pezzottify.remoteapi.model.BandArtist
import com.lelloman.pezzottify.remoteapi.model.IndividualArtist
import java.lang.reflect.Type

internal class ArtistTypeAdapter : JsonDeserializer<Artist> {
    override fun deserialize(
        json: JsonElement,
        typeOfT: Type?,
        context: JsonDeserializationContext
    ): Artist {
        if (json.asJsonObject?.get("members") != null) {
            return context.deserialize(json, BandArtist::class.java) as Artist
        }
        return context.deserialize(json, IndividualArtist::class.java) as Artist
    }
}