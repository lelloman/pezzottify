package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class AvailabilityCount(
    val total: Int,
    val available: Int,
    val unavailable: Int,
)

@Serializable
data class CatalogStatsCounts(
    val artists: AvailabilityCount,
    val albums: AvailabilityCount,
    val tracks: AvailabilityCount,
)

@Serializable
data class CatalogStatsResponse(
    @SerialName("computed_at")
    val computedAt: String,
    @SerialName("duration_ms")
    val durationMs: Long,
    val counts: CatalogStatsCounts,
)
