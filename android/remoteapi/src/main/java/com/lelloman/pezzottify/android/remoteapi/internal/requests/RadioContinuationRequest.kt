package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

@Serializable
data class RadioContinuationRequest(
    val source: String,
    val seed: RadioContinuationSeed,
    val settings: JsonObject? = null,
    @SerialName("context_track_ids") val contextTrackIds: List<String>,
    @SerialName("exclude_track_ids") val excludeTrackIds: List<String>,
    val count: Int = 10,
)

@Serializable
data class RadioContinuationSeed(
    @SerialName("entity_type") val entityType: String,
    @SerialName("entity_id") val entityId: String,
)
