package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Type of external search result.
 */
@Serializable
enum class ExternalSearchResultType {
    @SerialName("album")
    Album,
    @SerialName("artist")
    Artist,
}

/**
 * Request status information for an item in the download queue.
 * Used by ExternalSearchResult and ExternalAlbumDetailsResponse.
 */
@Serializable
data class RequestStatusInfo(
    /** Queue item ID (UUID) */
    @SerialName("request_id")
    val requestId: String,
    /** Current status in the queue */
    val status: DownloadQueueStatus,
    /** Position in queue (1-based, only for pending items) */
    @SerialName("queue_position")
    val queuePosition: Int? = null,
    /** Download progress (for album downloads with children) */
    val progress: DownloadProgress? = null,
    /** Error message (for failed items) */
    @SerialName("error_message")
    val errorMessage: String? = null,
    /** When the request was created (Unix timestamp) */
    @SerialName("created_at")
    val createdAt: Long,
)

/**
 * A single search result from the external downloader service.
 */
@Serializable
data class ExternalSearchResult(
    /** External ID from the music provider */
    val id: String,
    /** Result type ("album" or "artist") */
    @SerialName("type")
    val resultType: ExternalSearchResultType,
    /** Name of the album or artist */
    val name: String,
    /** Artist name (for albums) */
    @SerialName("artist_name")
    val artistName: String? = null,
    /** URL to cover/portrait image */
    @SerialName("image_url")
    val imageUrl: String? = null,
    /** Release year (for albums) */
    val year: Int? = null,
    /** Whether this content is already in the catalog */
    @SerialName("in_catalog")
    val inCatalog: Boolean = false,
    /** Whether this content is currently in the download queue */
    @SerialName("in_queue")
    val inQueue: Boolean = false,
    /** Relevance score (0.0 to 1.0, higher is better match) */
    val score: Float = 0f,
    /** Download request status if in queue (for discography results) */
    @SerialName("request_status")
    val requestStatus: RequestStatusInfo? = null,
)

/**
 * Collection of external search results from the server.
 */
@Serializable
data class ExternalSearchResponse(
    /** The search results */
    val results: List<ExternalSearchResult>,
    /** Total number of results (may be more than returned) */
    val total: Int,
)

/**
 * Artist discography response from the external downloader.
 * Used by GET /v1/download/search/discography/:artist_id
 */
@Serializable
data class ExternalDiscographyResponse(
    /** The artist information */
    val artist: ExternalSearchResult,
    /** All albums by this artist */
    val albums: List<ExternalSearchResult>,
)
