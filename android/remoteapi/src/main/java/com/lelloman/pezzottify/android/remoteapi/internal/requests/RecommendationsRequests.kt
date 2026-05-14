package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class ContinuationRecommendationsRequest(
    @SerialName("context_track_ids")
    val contextTrackIds: List<String>,
    @SerialName("exclude_track_ids")
    val excludeTrackIds: List<String>,
    val count: Int = 1,
)

@Serializable
data class TrackIdsResponse(
    @SerialName("track_ids")
    val trackIds: List<String>,
)
