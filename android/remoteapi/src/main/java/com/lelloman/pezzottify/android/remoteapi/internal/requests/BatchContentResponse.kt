package com.lelloman.pezzottify.android.remoteapi.internal.requests

import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonContentPolymorphicSerializer
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.jsonObject

/**
 * Response from batch content endpoint.
 * Maps content IDs to their fetch results.
 */
@Serializable
data class BatchContentResponse(
    val artists: Map<String, BatchArtistResult>,
    val albums: Map<String, BatchAlbumResult>,
    val tracks: Map<String, BatchTrackResult>,
)

/**
 * Result for a single artist in a batch response.
 * Either Ok with the data or Error with a message.
 */
@Serializable(with = BatchArtistResultSerializer::class)
sealed class BatchArtistResult {
    @Serializable
    data class Ok(val ok: ArtistResponse) : BatchArtistResult()

    @Serializable
    data class Error(val error: String) : BatchArtistResult()
}

/**
 * Result for a single album in a batch response.
 * Either Ok with the data or Error with a message.
 */
@Serializable(with = BatchAlbumResultSerializer::class)
sealed class BatchAlbumResult {
    @Serializable
    data class Ok(val ok: AlbumResponse) : BatchAlbumResult()

    @Serializable
    data class Error(val error: String) : BatchAlbumResult()
}

/**
 * Result for a single track in a batch response.
 * Either Ok with the data or Error with a message.
 */
@Serializable(with = BatchTrackResultSerializer::class)
sealed class BatchTrackResult {
    @Serializable
    data class Ok(val ok: TrackResponse) : BatchTrackResult()

    @Serializable
    data class Error(val error: String) : BatchTrackResult()
}

// Custom serializers to handle untagged union deserialization

internal object BatchArtistResultSerializer : JsonContentPolymorphicSerializer<BatchArtistResult>(BatchArtistResult::class) {
    override fun selectDeserializer(element: JsonElement): DeserializationStrategy<BatchArtistResult> {
        return when {
            "ok" in element.jsonObject -> BatchArtistResult.Ok.serializer()
            "error" in element.jsonObject -> BatchArtistResult.Error.serializer()
            else -> throw IllegalArgumentException("Unknown BatchArtistResult type: $element")
        }
    }
}

internal object BatchAlbumResultSerializer : JsonContentPolymorphicSerializer<BatchAlbumResult>(BatchAlbumResult::class) {
    override fun selectDeserializer(element: JsonElement): DeserializationStrategy<BatchAlbumResult> {
        return when {
            "ok" in element.jsonObject -> BatchAlbumResult.Ok.serializer()
            "error" in element.jsonObject -> BatchAlbumResult.Error.serializer()
            else -> throw IllegalArgumentException("Unknown BatchAlbumResult type: $element")
        }
    }
}

internal object BatchTrackResultSerializer : JsonContentPolymorphicSerializer<BatchTrackResult>(BatchTrackResult::class) {
    override fun selectDeserializer(element: JsonElement): DeserializationStrategy<BatchTrackResult> {
        return when {
            "ok" in element.jsonObject -> BatchTrackResult.Ok.serializer()
            "error" in element.jsonObject -> BatchTrackResult.Error.serializer()
            else -> throw IllegalArgumentException("Unknown BatchTrackResult type: $element")
        }
    }
}
