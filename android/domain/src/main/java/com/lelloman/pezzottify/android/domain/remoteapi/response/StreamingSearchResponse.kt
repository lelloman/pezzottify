package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Section types for streaming search SSE response.
 * Each section is sent as a separate SSE event.
 */

// Summary types used in enrichment sections

@Serializable
data class TrackSummary(
    val id: String,
    val name: String,
    @SerialName("duration_ms")
    val durationMs: Long,
    @SerialName("track_number")
    val trackNumber: Int? = null,
    @SerialName("album_id")
    val albumId: String,
    @SerialName("album_name")
    val albumName: String,
    @SerialName("artist_names")
    val artistNames: List<String>,
    @SerialName("image_id")
    val imageId: String? = null,
)

@Serializable
data class AlbumSummary(
    val id: String,
    val name: String,
    @SerialName("release_year")
    val releaseYear: Int? = null,
    @SerialName("track_count")
    val trackCount: Int,
    @SerialName("image_id")
    val imageId: String? = null,
    @SerialName("artist_names")
    val artistNames: List<String>,
)

@Serializable
data class ArtistSummary(
    val id: String,
    val name: String,
    @SerialName("image_id")
    val imageId: String? = null,
)

// Search result types (used in primary matches and more_results/results sections)

@Serializable
data class SearchedTrack(
    val id: String,
    val name: String,
    val duration: Int,
    @SerialName("artists_ids_names")
    val artistsIdsNames: List<List<String>>,
    @SerialName("image_id")
    val imageId: String? = null,
    @SerialName("album_id")
    val albumId: String,
    val availability: String,
)

@Serializable
data class SearchedAlbum(
    val id: String,
    val name: String,
    @SerialName("artists_ids_names")
    val artistsIdsNames: List<List<String>>,
    @SerialName("image_id")
    val imageId: String? = null,
    val year: Long? = null,
)

@Serializable
data class SearchedArtist(
    val id: String,
    val name: String,
    @SerialName("image_id")
    val imageId: String? = null,
)

/**
 * Resolved search result - internally tagged union with "type" discriminator.
 * Server uses Rust's #[serde(tag = "type")] format.
 */
@Serializable
@kotlinx.serialization.json.JsonClassDiscriminator("type")
sealed class ResolvedSearchResult {
    @Serializable
    @SerialName("Track")
    data class Track(
        val id: String,
        val name: String,
        val duration: Int,
        @SerialName("artists_ids_names")
        val artistsIdsNames: List<List<String>>,
        @SerialName("image_id")
        val imageId: String? = null,
        @SerialName("album_id")
        val albumId: String,
        val availability: String,
    ) : ResolvedSearchResult()

    @Serializable
    @SerialName("Album")
    data class Album(
        val id: String,
        val name: String,
        @SerialName("artists_ids_names")
        val artistsIdsNames: List<List<String>>,
        @SerialName("image_id")
        val imageId: String? = null,
        val year: Long? = null,
    ) : ResolvedSearchResult()

    @Serializable
    @SerialName("Artist")
    data class Artist(
        val id: String,
        val name: String,
        @SerialName("image_id")
        val imageId: String? = null,
    ) : ResolvedSearchResult()
}

/**
 * Match type enum for enrichment sections.
 */
@Serializable
enum class MatchType {
    @SerialName("artist")
    Artist,
    @SerialName("album")
    Album,
    @SerialName("track")
    Track,
}

/**
 * Streaming search section - one section per SSE event.
 * Uses polymorphic serialization with "section" as the type discriminator.
 */
@Serializable
sealed class SearchSection {
    /**
     * High-confidence artist match with enrichment.
     */
    @Serializable
    @SerialName("primary_artist")
    data class PrimaryArtist(
        val item: ResolvedSearchResult,
        val confidence: Double,
    ) : SearchSection()

    /**
     * High-confidence album match with enrichment.
     */
    @Serializable
    @SerialName("primary_album")
    data class PrimaryAlbum(
        val item: ResolvedSearchResult,
        val confidence: Double,
    ) : SearchSection()

    /**
     * High-confidence track match.
     */
    @Serializable
    @SerialName("primary_track")
    data class PrimaryTrack(
        val item: ResolvedSearchResult,
        val confidence: Double,
    ) : SearchSection()

    /**
     * Popular tracks by the target artist.
     */
    @Serializable
    @SerialName("popular_by")
    data class PopularBy(
        @SerialName("target_id")
        val targetId: String,
        @SerialName("target_type")
        val targetType: MatchType,
        val items: List<TrackSummary>,
    ) : SearchSection()

    /**
     * Albums by the target artist.
     */
    @Serializable
    @SerialName("albums_by")
    data class AlbumsBy(
        @SerialName("target_id")
        val targetId: String,
        val items: List<AlbumSummary>,
    ) : SearchSection()

    /**
     * Tracks from the target album.
     */
    @Serializable
    @SerialName("tracks_from")
    data class TracksFrom(
        @SerialName("target_id")
        val targetId: String,
        val items: List<TrackSummary>,
    ) : SearchSection()

    /**
     * Related artists (from artist metadata).
     */
    @Serializable
    @SerialName("related_artists")
    data class RelatedArtists(
        @SerialName("target_id")
        val targetId: String,
        val items: List<ArtistSummary>,
    ) : SearchSection()

    /**
     * Remaining results (shown when there's at least one primary match).
     */
    @Serializable
    @SerialName("more_results")
    data class MoreResults(
        val items: List<ResolvedSearchResult>,
    ) : SearchSection()

    /**
     * All results (shown when there are no primary matches).
     */
    @Serializable
    @SerialName("results")
    data class Results(
        val items: List<ResolvedSearchResult>,
    ) : SearchSection()

    /**
     * Stream complete.
     */
    @Serializable
    @SerialName("done")
    data class Done(
        @SerialName("total_time_ms")
        val totalTimeMs: Long,
    ) : SearchSection()
}
