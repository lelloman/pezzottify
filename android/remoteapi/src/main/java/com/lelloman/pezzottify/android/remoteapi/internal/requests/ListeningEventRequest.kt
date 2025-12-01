package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal data class ListeningEventRequest(
    @SerialName("track_id") val trackId: String,
    @SerialName("session_id") val sessionId: String,
    @SerialName("started_at") val startedAt: Long,
    @SerialName("ended_at") val endedAt: Long?,
    @SerialName("duration_seconds") val durationSeconds: Int,
    @SerialName("track_duration_seconds") val trackDurationSeconds: Int,
    @SerialName("seek_count") val seekCount: Int,
    @SerialName("pause_count") val pauseCount: Int,
    @SerialName("playback_context") val playbackContext: String,
    @SerialName("client_type") val clientType: String,
)

@Serializable
internal data class ListeningEventResponse(
    val id: Long,
    val created: Boolean,
)
