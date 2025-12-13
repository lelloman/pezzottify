package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Individual listening event with full details.
 * Response from GET /v1/user/listening/events returns List<ListeningEventItem> directly.
 */
@Serializable
data class ListeningEventItem(
    val id: Long,
    @SerialName("user_id")
    val userId: Long,
    @SerialName("track_id")
    val trackId: String,
    @SerialName("session_id")
    val sessionId: String?,
    @SerialName("started_at")
    val startedAt: Long,
    @SerialName("ended_at")
    val endedAt: Long?,
    @SerialName("duration_seconds")
    val durationSeconds: Int,
    @SerialName("track_duration_seconds")
    val trackDurationSeconds: Int,
    val completed: Boolean,
    @SerialName("seek_count")
    val seekCount: Int,
    @SerialName("pause_count")
    val pauseCount: Int,
    @SerialName("playback_context")
    val playbackContext: String?,
    @SerialName("client_type")
    val clientType: String?,
    val date: Int,
)
