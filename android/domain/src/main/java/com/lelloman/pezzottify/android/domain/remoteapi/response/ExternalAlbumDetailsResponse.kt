package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Track information from an external album.
 */
@Serializable
data class ExternalTrackInfo(
    /** External track ID */
    val id: String,
    /** Track name */
    val name: String,
    /** Track number within the disc */
    @SerialName("track_number")
    val trackNumber: Int,
    /** Disc number (for multi-disc albums) */
    @SerialName("disc_number")
    val discNumber: Int? = null,
    /** Duration in milliseconds */
    @SerialName("duration_ms")
    val durationMs: Long? = null,
)

/**
 * Detailed information about an external album.
 * Response from GET /v1/download/album/:album_id
 */
@Serializable
data class ExternalAlbumDetailsResponse(
    /** External album ID */
    val id: String,
    /** Album name */
    val name: String,
    /** Primary artist ID */
    @SerialName("artist_id")
    val artistId: String,
    /** Primary artist name */
    @SerialName("artist_name")
    val artistName: String,
    /** URL to cover image */
    @SerialName("image_url")
    val imageUrl: String? = null,
    /** Release year */
    val year: Int? = null,
    /** Album type: "album", "single", "ep", "compilation" */
    @SerialName("album_type")
    val albumType: String? = null,
    /** Total number of tracks on the album */
    @SerialName("total_tracks")
    val totalTracks: Int,
    /** List of tracks on the album */
    val tracks: List<ExternalTrackInfo>,
    /** Whether this album is already in the local catalog */
    @SerialName("in_catalog")
    val inCatalog: Boolean,
    /** Request status if the album is in the download queue */
    @SerialName("request_status")
    val requestStatus: RequestStatusInfo? = null,
)
