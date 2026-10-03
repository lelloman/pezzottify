package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.EncodeDefault
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Body of `POST /v1/content/recommendations/continuation`.
 *
 * The shared Retrofit converter encodes defaults, so nullable knobs use
 * [EncodeDefault.Mode.NEVER]: a null is omitted and the server applies its own default.
 * List fields always encode (an empty list is what the server's `#[serde(default)]` expects).
 */
@OptIn(ExperimentalSerializationApi::class)
@Serializable
data class ContinuationRecommendationsRequest(
    @SerialName("context_track_ids")
    val contextTrackIds: List<String> = emptyList(),
    @SerialName("exclude_track_ids")
    val excludeTrackIds: List<String> = emptyList(),
    val count: Int = 1,
    @SerialName("source_track_ids")
    val sourceTrackIds: List<String> = emptyList(),
    @SerialName("source_references")
    val sourceReferences: List<ContinuationReferenceRequest> = emptyList(),
    @SerialName("recent_track_ids")
    val recentTrackIds: List<String> = emptyList(),
    @EncodeDefault(EncodeDefault.Mode.NEVER)
    @SerialName("recency_weight")
    val recencyWeight: Double? = null,
    /** The destination mix; omitted when not steering. */
    @EncodeDefault(EncodeDefault.Mode.NEVER)
    val destination: List<ContinuationReferenceRequest>? = null,
    @EncodeDefault(EncodeDefault.Mode.NEVER)
    val progress: Double? = null,
    @EncodeDefault(EncodeDefault.Mode.NEVER)
    val criteria: List<ContinuationCriterionRequest>? = null,
    @EncodeDefault(EncodeDefault.Mode.NEVER)
    val diversity: Double? = null,
    @EncodeDefault(EncodeDefault.Mode.NEVER)
    val randomness: Double? = null,
    @EncodeDefault(EncodeDefault.Mode.NEVER)
    val mode: String? = null,
    val away: List<ContinuationReferenceRequest> = emptyList(),
)

@OptIn(ExperimentalSerializationApi::class)
@Serializable
data class ContinuationReferenceRequest(
    @SerialName("entity_type")
    val entityType: String,
    @SerialName("entity_id")
    val entityId: String,
    @EncodeDefault(EncodeDefault.Mode.NEVER)
    val weight: Double? = null,
)

@Serializable
data class ContinuationCriterionRequest(
    val namespace: String,
    val weight: Double,
)

@Serializable
data class ContinuationResponse(
    @SerialName("track_ids")
    val trackIds: List<String>,
    @SerialName("recency_weight")
    val recencyWeight: Double? = null,
    val progress: Double? = null,
    val namespaces: List<ContinuationNamespaceResponse> = emptyList(),
)

@Serializable
data class ContinuationNamespaceResponse(
    val namespace: String,
    val weight: Double = 1.0,
    @SerialName("source_to_destination")
    val sourceToDestination: Double? = null,
    @SerialName("query_to_source")
    val queryToSource: Double? = null,
    @SerialName("query_to_destination")
    val queryToDestination: Double? = null,
    @SerialName("destination_components")
    val destinationComponents: List<ContinuationComponentResponse> = emptyList(),
)

@Serializable
data class ContinuationComponentResponse(
    @SerialName("entity_type")
    val entityType: String,
    @SerialName("entity_id")
    val entityId: String,
    val similarity: Double? = null,
)

@Serializable
data class TrackIdsResponse(
    @SerialName("track_ids")
    val trackIds: List<String>,
)
