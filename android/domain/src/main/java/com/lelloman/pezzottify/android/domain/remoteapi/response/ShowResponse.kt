package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.Serializable

@Serializable
data class ShowSummaryResponse(
    val id: String,
    val title: String,
    val status: String,
    val summary: String,
    val language: String,
    val targetDurationMinutes: Int,
    val createdAt: Long,
    val updatedAt: Long,
    val publishedAt: Long? = null,
    val segmentCount: Int,
    val trackCount: Int,
)

@Serializable
data class ShowResponse(
    val id: String,
    val title: String,
    val status: String,
    val brief: String,
    val summary: String,
    val language: String,
    val targetDurationMinutes: Int,
    val createdByUserId: Int,
    val createdAt: Long,
    val updatedAt: Long,
    val publishedAt: Long? = null,
    val speakers: List<ShowSpeakerResponse> = emptyList(),
    val segments: List<ShowSegmentResponse> = emptyList(),
    val sources: List<ShowSourceResponse> = emptyList(),
    val error: String? = null,
)

@Serializable
data class ShowSpeakerResponse(
    val id: String,
    val name: String,
    val voiceId: String? = null,
)

@Serializable
data class ShowSourceResponse(
    val id: String,
    val title: String,
    val url: String? = null,
    val excerpt: String? = null,
)

@Serializable
data class ShowSegmentResponse(
    val id: String,
    val kind: String,
    val title: String,
    val trackId: String? = null,
    val speakerId: String? = null,
    val text: String? = null,
    val audioPath: String? = null,
    val mimeType: String? = null,
    val durationMs: Long? = null,
    val sourceIds: List<String> = emptyList(),
)
