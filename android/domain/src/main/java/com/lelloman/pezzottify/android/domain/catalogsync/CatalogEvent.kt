package com.lelloman.pezzottify.android.domain.catalogsync

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Type of catalog event that occurred.
 */
@Serializable
enum class CatalogEventType {
    @SerialName("album_updated")
    AlbumUpdated,

    @SerialName("artist_updated")
    ArtistUpdated,

    @SerialName("track_updated")
    TrackUpdated,

    @SerialName("album_added")
    AlbumAdded,

    @SerialName("artist_added")
    ArtistAdded,

    @SerialName("track_added")
    TrackAdded,
}

/**
 * Content type for catalog events.
 */
@Serializable
enum class CatalogContentType {
    @SerialName("album")
    Album,

    @SerialName("artist")
    Artist,

    @SerialName("track")
    Track,
}

/**
 * A catalog invalidation event.
 *
 * Represents a change to catalog content that clients should respond to
 * by invalidating their cached data.
 */
@Serializable
data class CatalogEvent(
    /** Sequence number for ordering and catch-up. */
    val seq: Long,

    /** Type of event. */
    @SerialName("event_type")
    val eventType: CatalogEventType,

    /** Type of content affected. */
    @SerialName("content_type")
    val contentType: CatalogContentType,

    /** ID of the affected content. */
    @SerialName("content_id")
    val contentId: String,

    /** Unix timestamp when the event occurred. */
    val timestamp: Long,

    /** What triggered this event (e.g., "download_completion", "ingestion", "admin_edit"). */
    @SerialName("triggered_by")
    val triggeredBy: String? = null,
)

/**
 * Response from GET /v1/sync/catalog endpoint.
 */
@Serializable
data class CatalogSyncResponse(
    /** List of events since the requested sequence number. */
    val events: List<CatalogEvent>,

    /** Current (highest) sequence number. */
    @SerialName("current_seq")
    val currentSeq: Long,
)
