package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Response for GET /v1/catalog/skeleton/version
 */
@Serializable
data class SkeletonVersionResponse(
    val version: Long,
    val checksum: String
)

/**
 * Response for GET /v1/catalog/skeleton
 */
@Serializable
data class FullSkeletonResponse(
    val version: Long,
    val checksum: String,
    val artists: List<String>,
    val albums: List<SkeletonAlbumDto>,
    val tracks: List<SkeletonTrackDto>
)

/**
 * Album entry in skeleton response.
 */
@Serializable
data class SkeletonAlbumDto(
    val id: String,
    @SerialName("artist_ids") val artistIds: List<String>
)

/**
 * Track entry in skeleton response.
 */
@Serializable
data class SkeletonTrackDto(
    val id: String,
    @SerialName("album_id") val albumId: String
)

/**
 * Response for GET /v1/catalog/skeleton/delta
 */
@Serializable
data class SkeletonDeltaResponse(
    @SerialName("from_version") val fromVersion: Long,
    @SerialName("to_version") val toVersion: Long,
    val checksum: String,
    val changes: List<SkeletonChangeDto>
)

/**
 * A skeleton change in delta response.
 */
@Serializable
data class SkeletonChangeDto(
    val type: String,
    val id: String,
    @SerialName("artist_ids") val artistIds: List<String>? = null,
    @SerialName("album_id") val albumId: String? = null
)
