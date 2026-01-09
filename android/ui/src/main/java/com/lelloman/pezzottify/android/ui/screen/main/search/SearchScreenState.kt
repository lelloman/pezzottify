package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.annotation.StringRes
import com.lelloman.pezzottify.android.ui.content.AlbumAvailability
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.screen.main.home.ResolvedRecentlyViewedContent
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import kotlinx.coroutines.flow.Flow

data class SearchScreenState(
    val query: String = "",
    val isLoading: Boolean = false,
    val searchResults: List<Flow<Content<SearchResultContent>>>? = null,
    @StringRes val searchErrorRes: Int? = null,
    val recentlyViewedContent: List<Flow<Content<ResolvedRecentlyViewedContent>>>? = null,
    val searchHistoryItems: List<Flow<Content<SearchHistoryItem>>>? = null,
    val selectedFilters: Set<SearchFilter> = emptySet(),
    val whatsNewContent: WhatsNewContentState? = null,
    // Streaming search state
    val isStreamingSearchEnabled: Boolean = false,
    val streamingSections: List<StreamingSearchSection> = emptyList(),
)

/**
 * State for the What's New section, showing recently added albums grouped by batch.
 */
data class WhatsNewContentState(
    val albums: List<WhatsNewAlbumGroup>,
    val isLoading: Boolean = false,
)

/**
 * A group of albums from a single batch.
 */
data class WhatsNewAlbumGroup(
    val batchId: String,
    val batchName: String,
    val closedAt: Long,
    val albums: List<Flow<Content<WhatsNewAlbumItem>>>,
)

/**
 * A resolved album item for the What's New widget.
 */
data class WhatsNewAlbumItem(
    val id: String,
    val name: String,
    val imageUrl: String?,
    val artistIds: List<String>,
)

data class SearchHistoryItem(
    val query: String,
    val contentId: String,
    val contentName: String,
    val contentImageUrl: String?,
    val contentType: ViewedContentType,
)

/**
 * Streaming search section types - UI representation of server SearchSection.
 * These are progressively populated as the SSE stream emits events.
 */
sealed class StreamingSearchSection {
    /**
     * High-confidence primary match (artist, album, or track).
     */
    data class PrimaryMatch(
        val id: String,
        val name: String,
        val type: PrimaryMatchType,
        val imageUrl: String?,
        val confidence: Double,
        // Additional context based on type
        val artistNames: List<String>? = null,  // For album/track
        val year: Int? = null,  // For album
        val durationMs: Long? = null,  // For track
        val albumName: String? = null,  // For track
    ) : StreamingSearchSection()

    /**
     * Popular tracks by the target artist.
     */
    data class PopularTracks(
        val targetId: String,
        val tracks: List<StreamingTrackSummary>,
    ) : StreamingSearchSection()

    /**
     * Albums by the target artist.
     */
    data class ArtistAlbums(
        val targetId: String,
        val albums: List<StreamingAlbumSummary>,
    ) : StreamingSearchSection()

    /**
     * Tracks from the target album.
     */
    data class AlbumTracks(
        val targetId: String,
        val tracks: List<StreamingTrackSummary>,
    ) : StreamingSearchSection()

    /**
     * Related artists.
     */
    data class RelatedArtists(
        val targetId: String,
        val artists: List<StreamingArtistSummary>,
    ) : StreamingSearchSection()

    /**
     * Additional search results (when there are primary matches).
     */
    data class MoreResults(
        val results: List<StreamingSearchResult>,
    ) : StreamingSearchSection()

    /**
     * All search results (when there are no primary matches).
     */
    data class AllResults(
        val results: List<StreamingSearchResult>,
    ) : StreamingSearchSection()

    /**
     * Search completed.
     */
    data class Done(
        val totalTimeMs: Long,
    ) : StreamingSearchSection()
}

enum class PrimaryMatchType {
    Artist,
    Album,
    Track,
}

data class StreamingTrackSummary(
    val id: String,
    val name: String,
    val durationMs: Long,
    val trackNumber: Int?,
    val albumId: String,
    val albumName: String,
    val artistNames: List<String>,
    val imageUrl: String?,
)

data class StreamingAlbumSummary(
    val id: String,
    val name: String,
    val releaseYear: Int?,
    val trackCount: Int,
    val imageUrl: String?,
    val artistNames: List<String>,
    val availability: AlbumAvailability = AlbumAvailability.Complete,
)

data class StreamingArtistSummary(
    val id: String,
    val name: String,
    val imageUrl: String?,
)

sealed class StreamingSearchResult {
    data class Artist(
        val id: String,
        val name: String,
        val imageUrl: String?,
    ) : StreamingSearchResult()

    data class Album(
        val id: String,
        val name: String,
        val artistNames: List<String>,
        val imageUrl: String?,
        val year: Int?,
        val availability: AlbumAvailability = AlbumAvailability.Complete,
    ) : StreamingSearchResult()

    data class Track(
        val id: String,
        val name: String,
        val artistNames: List<String>,
        val imageUrl: String?,
        val albumId: String,
        val durationMs: Long,
    ) : StreamingSearchResult()
}