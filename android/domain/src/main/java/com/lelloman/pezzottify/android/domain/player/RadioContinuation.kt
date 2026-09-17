package com.lelloman.pezzottify.android.domain.player

import java.util.UUID
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject

@Serializable
data class RadioContinuation(
    @SerialName("session_id") val sessionId: String = UUID.randomUUID().toString(),
    val strategy: String = "seeded_radio",
    val status: String = "active",
    @SerialName("seen_track_ids") val seenTrackIds: List<String> = emptyList(),
    @SerialName("ordered_track_ids") val orderedTrackIds: List<String> = emptyList(),
    @SerialName("next_index") val nextIndex: Int = 0,
) {
    // Include defaults: the web client consumes this as plain JSON, not a Kotlin data class.
    fun toWireJson(): JsonObject = wireJson.encodeToJsonElement(serializer(), this).jsonObject

    fun edited(trackIds: List<String>, keepGoing: Boolean) = copy(
        status = if (keepGoing) status else "stopped",
        seenTrackIds = (seenTrackIds + trackIds).distinct(),
    )

    fun nextBatch(): Pair<List<String>, Int> {
        val seen = seenTrackIds.toHashSet()
        val batch = mutableListOf<String>()
        var cursor = nextIndex.coerceIn(0, orderedTrackIds.size)
        while (cursor < orderedTrackIds.size && batch.size < 10) {
            val id = orderedTrackIds[cursor++]
            if (seen.add(id)) batch.add(id)
        }
        return batch to cursor
    }

    fun appended(trackIds: List<String>, cursor: Int = nextIndex) = copy(
        seenTrackIds = (seenTrackIds + trackIds).distinct(),
        nextIndex = cursor,
        status = if (trackIds.isEmpty() || (strategy == "ranked_snapshot" && cursor >= orderedTrackIds.size)) "exhausted" else "active",
    )

    companion object {
        private val wireJson = Json { encodeDefaults = true }
        fun create(trackIds: List<String>, snapshot: List<String>? = null) = RadioContinuation(
            strategy = if (snapshot == null) "seeded_radio" else "ranked_snapshot",
            status = if (snapshot != null && trackIds.size >= snapshot.size) "exhausted" else "active",
            seenTrackIds = trackIds.distinct(),
            orderedTrackIds = snapshot.orEmpty(),
            nextIndex = if (snapshot == null) 0 else trackIds.size,
        )
    }
}
