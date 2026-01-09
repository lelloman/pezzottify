package com.lelloman.pezzottify.android.domain.skeleton

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Result of fetching a page of discography.
 */
sealed class FetchPageResult {
    data class Success(val fetchedCount: Int, val hasMore: Boolean) : FetchPageResult()
    data class Error(val message: String) : FetchPageResult()
    data object AlreadyComplete : FetchPageResult()
}

/**
 * State of discography for an artist.
 */
data class DiscographyState(
    val albumIds: List<String>,
    val totalOnServer: Int?,
    val isLoading: Boolean,
    val hasMore: Boolean,
    val error: String?
)

/**
 * On-demand discography fetcher with pagination support.
 *
 * Always fetches fresh data from the server to ensure proper ordering
 * (by availability, then popularity/date). The local DB acts as a cache
 * for offline access but is refreshed on each screen visit.
 *
 * Usage:
 * 1. Call observeDiscography(artistId) to get a Flow of current state
 * 2. Call fetchFirstPage(artistId) when artist screen opens
 * 3. Call fetchMoreAlbums(artistId) when user scrolls to load more
 */
@Singleton
class DiscographyCacheFetcher @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val skeletonStore: SkeletonStore,
    loggerFactory: LoggerFactory,
) {

    private val logger: Logger by loggerFactory

    companion object {
        const val PAGE_SIZE = 20
    }

    // Track server totals per artist (so we know if there's more to fetch)
    private val serverTotals = MutableStateFlow<Map<String, Int>>(emptyMap())

    // Track current offset per artist (how many albums we've fetched from server)
    private val currentOffsets = MutableStateFlow<Map<String, Int>>(emptyMap())

    // Track loading state per artist
    private val loadingStates = MutableStateFlow<Map<String, Boolean>>(emptyMap())

    // Track errors per artist
    private val errors = MutableStateFlow<Map<String, String?>>(emptyMap())

    // Mutex per artist to prevent concurrent fetches
    private val fetchMutexes = mutableMapOf<String, Mutex>()

    private fun getMutex(artistId: String): Mutex {
        return synchronized(fetchMutexes) {
            fetchMutexes.getOrPut(artistId) { Mutex() }
        }
    }

    /**
     * Observe discography state for an artist.
     *
     * Returns a Flow that emits:
     * - Current cached album IDs from local DB (ordered by server sort)
     * - Whether there are more albums on server
     * - Loading state
     * - Any errors
     */
    fun observeDiscography(artistId: String): Flow<DiscographyState> {
        return combine(
            skeletonStore.observeAlbumIdsForArtist(artistId),
            serverTotals,
            currentOffsets,
            loadingStates,
            errors
        ) { albumIds, totals, offsets, loading, errs ->
            val total = totals[artistId]
            val offset = offsets[artistId] ?: 0
            DiscographyState(
                albumIds = albumIds,
                totalOnServer = total,
                isLoading = loading[artistId] == true,
                hasMore = total != null && offset < total,
                error = errs[artistId]
            )
        }
    }

    /**
     * Fetch the first page of albums for an artist.
     *
     * This clears existing cached albums and fetches fresh data from the server
     * to ensure proper ordering (by availability, then popularity/date).
     * Call this when the artist screen opens.
     */
    suspend fun fetchFirstPage(artistId: String): FetchPageResult = withContext(Dispatchers.IO) {
        val mutex = getMutex(artistId)

        // Prevent concurrent fetches for same artist
        if (!mutex.tryLock()) {
            logger.debug("fetchFirstPage($artistId) already in progress, skipping")
            return@withContext FetchPageResult.Success(0, true)
        }

        try {
            setLoading(artistId, true)
            clearError(artistId)

            logger.info("fetchFirstPage($artistId) pageSize=$PAGE_SIZE")

            // Fetch first page from server
            when (val response = remoteApiClient.getArtistDiscography(
                artistId = artistId,
                offset = 0,
                limit = PAGE_SIZE
            )) {
                is RemoteApiResponse.Success -> {
                    val data = response.data

                    // Update server total and reset offset
                    setServerTotal(artistId, data.total)
                    setCurrentOffset(artistId, data.albums.size)

                    // Clear existing albums and insert fresh data with order index
                    skeletonStore.deleteAlbumsForArtist(artistId)

                    val albumArtists = data.albums.mapIndexed { index, album ->
                        AlbumArtistRelationship(
                            artistId = artistId,
                            albumId = album.id,
                            orderIndex = index
                        )
                    }
                    skeletonStore.insertAlbumArtists(albumArtists)

                    val hasMore = data.albums.size < data.total

                    logger.info("fetchFirstPage($artistId) fetched ${data.albums.size} albums, total=${data.total}, hasMore=$hasMore")

                    FetchPageResult.Success(
                        fetchedCount = data.albums.size,
                        hasMore = hasMore
                    )
                }
                is RemoteApiResponse.Error -> {
                    val errorMsg = "Failed to fetch discography: $response"
                    logger.error("fetchFirstPage($artistId) $errorMsg")
                    setError(artistId, errorMsg)
                    FetchPageResult.Error(errorMsg)
                }
            }
        } finally {
            setLoading(artistId, false)
            mutex.unlock()
        }
    }

    /**
     * Fetch more albums for an artist (next page).
     *
     * Call this when the user scrolls down to load more albums.
     * Albums are appended to the existing cache with correct order indices.
     */
    suspend fun fetchMoreAlbums(artistId: String): FetchPageResult = withContext(Dispatchers.IO) {
        val mutex = getMutex(artistId)

        // Prevent concurrent fetches for same artist
        if (!mutex.tryLock()) {
            logger.debug("fetchMoreAlbums($artistId) already in progress, skipping")
            return@withContext FetchPageResult.Success(0, true)
        }

        try {
            setLoading(artistId, true)
            clearError(artistId)

            val offset = currentOffsets.value[artistId] ?: 0
            val total = serverTotals.value[artistId]

            // Check if we already have all albums
            if (total != null && offset >= total) {
                logger.info("fetchMoreAlbums($artistId) already complete: offset=$offset >= total=$total")
                return@withContext FetchPageResult.AlreadyComplete
            }

            logger.info("fetchMoreAlbums($artistId) offset=$offset, pageSize=$PAGE_SIZE")

            // Fetch next page from server
            when (val response = remoteApiClient.getArtistDiscography(
                artistId = artistId,
                offset = offset,
                limit = PAGE_SIZE
            )) {
                is RemoteApiResponse.Success -> {
                    val data = response.data

                    // Update server total and offset
                    setServerTotal(artistId, data.total)
                    val newOffset = offset + data.albums.size
                    setCurrentOffset(artistId, newOffset)

                    // Append albums with correct order index
                    val albumArtists = data.albums.mapIndexed { index, album ->
                        AlbumArtistRelationship(
                            artistId = artistId,
                            albumId = album.id,
                            orderIndex = offset + index
                        )
                    }
                    skeletonStore.insertAlbumArtists(albumArtists)

                    val hasMore = newOffset < data.total

                    logger.info("fetchMoreAlbums($artistId) fetched ${data.albums.size} albums, offset=$newOffset/${data.total}, hasMore=$hasMore")

                    FetchPageResult.Success(
                        fetchedCount = data.albums.size,
                        hasMore = hasMore
                    )
                }
                is RemoteApiResponse.Error -> {
                    val errorMsg = "Failed to fetch more albums: $response"
                    logger.error("fetchMoreAlbums($artistId) $errorMsg")
                    setError(artistId, errorMsg)
                    FetchPageResult.Error(errorMsg)
                }
            }
        } finally {
            setLoading(artistId, false)
            mutex.unlock()
        }
    }

    private fun setServerTotal(artistId: String, total: Int) {
        serverTotals.value = serverTotals.value + (artistId to total)
    }

    private fun setCurrentOffset(artistId: String, offset: Int) {
        currentOffsets.value = currentOffsets.value + (artistId to offset)
    }

    private fun setLoading(artistId: String, loading: Boolean) {
        loadingStates.value = if (loading) {
            loadingStates.value + (artistId to true)
        } else {
            loadingStates.value - artistId
        }
    }

    private fun setError(artistId: String, error: String) {
        errors.value = errors.value + (artistId to error)
    }

    private fun clearError(artistId: String) {
        errors.value = errors.value - artistId
    }
}
