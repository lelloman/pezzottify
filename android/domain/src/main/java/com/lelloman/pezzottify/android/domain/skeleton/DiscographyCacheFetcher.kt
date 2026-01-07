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
import kotlinx.coroutines.sync.withLock
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
    val error: String?
) {
    val hasMore: Boolean
        get() = totalOnServer != null && albumIds.size < totalOnServer
}

/**
 * On-demand discography fetcher with pagination support.
 *
 * Usage:
 * 1. Call observeDiscography(artistId) to get a Flow of current state
 * 2. Call fetchAllDiscography(artistId) when artist screen opens to fetch all album IDs
 */
@Singleton
class DiscographyCacheFetcher @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val skeletonStore: SkeletonStore,
    loggerFactory: LoggerFactory,
) {

    private val logger: Logger by loggerFactory

    companion object {
        const val PAGE_SIZE = 50
    }

    // Track server totals per artist (so we know if there's more to fetch)
    private val serverTotals = MutableStateFlow<Map<String, Int>>(emptyMap())

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
     * - Current cached album IDs from local DB
     * - Whether there are more albums on server
     * - Loading state
     * - Any errors
     */
    fun observeDiscography(artistId: String): Flow<DiscographyState> {
        return combine(
            skeletonStore.observeAlbumIdsForArtist(artistId),
            serverTotals,
            loadingStates,
            errors
        ) { albumIds, totals, loading, errs ->
            DiscographyState(
                albumIds = albumIds,
                totalOnServer = totals[artistId],
                isLoading = loading[artistId] == true,
                error = errs[artistId]
            )
        }
    }

    /**
     * Fetch the next page of albums for an artist.
     *
     * Albums are automatically stored in local DB and the Flow from
     * observeDiscography will emit the updated list.
     */
    suspend fun fetchNextPage(artistId: String): FetchPageResult = withContext(Dispatchers.IO) {
        val mutex = getMutex(artistId)

        // Prevent concurrent fetches for same artist
        if (!mutex.tryLock()) {
            logger.debug("fetchNextPage($artistId) already in progress, skipping")
            return@withContext FetchPageResult.Success(0, true)
        }

        try {
            setLoading(artistId, true)
            clearError(artistId)

            // Get current cached count
            val cachedAlbums = skeletonStore.getAlbumIdsForArtist(artistId)
            val offset = cachedAlbums.size

            logger.info("fetchNextPage($artistId) offset=$offset, pageSize=$PAGE_SIZE")

            // Fetch page from server
            when (val response = remoteApiClient.getArtistDiscography(
                artistId = artistId,
                offset = offset,
                limit = PAGE_SIZE
            )) {
                is RemoteApiResponse.Success -> {
                    val data = response.data

                    // Update server total
                    setServerTotal(artistId, data.total)

                    // Check if we're already complete
                    if (offset >= data.total) {
                        logger.info("fetchNextPage($artistId) already complete: $offset >= ${data.total}")
                        return@withContext FetchPageResult.AlreadyComplete
                    }

                    // Store albums in local DB
                    val albumArtists = data.albums.map { album ->
                        AlbumArtistRelationship(
                            artistId = artistId,
                            albumId = album.id
                        )
                    }
                    skeletonStore.insertAlbumArtists(albumArtists)

                    val newTotal = offset + albumArtists.size
                    val hasMore = newTotal < data.total

                    logger.info("fetchNextPage($artistId) fetched ${albumArtists.size} albums, total=$newTotal/${data.total}, hasMore=$hasMore")

                    FetchPageResult.Success(
                        fetchedCount = albumArtists.size,
                        hasMore = hasMore
                    )
                }
                is RemoteApiResponse.Error -> {
                    val errorMsg = "Failed to fetch discography: $response"
                    logger.error("fetchNextPage($artistId) $errorMsg")
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
     * Fetch ALL album IDs for an artist in a loop.
     *
     * Call this when artist screen opens to ensure full skeleton is cached.
     * Fetches in batches of PAGE_SIZE until local count = server total.
     */
    suspend fun fetchAllDiscography(artistId: String) = withContext(Dispatchers.IO) {
        logger.info("fetchAllDiscography($artistId) starting")

        var pageCount = 0
        while (true) {
            pageCount++
            when (val result = fetchNextPage(artistId)) {
                is FetchPageResult.Success -> {
                    if (!result.hasMore) {
                        logger.info("fetchAllDiscography($artistId) complete after $pageCount pages")
                        return@withContext
                    }
                    // Continue to next page
                }
                is FetchPageResult.AlreadyComplete -> {
                    logger.info("fetchAllDiscography($artistId) already complete")
                    return@withContext
                }
                is FetchPageResult.Error -> {
                    logger.error("fetchAllDiscography($artistId) failed on page $pageCount: ${result.message}")
                    return@withContext
                }
            }
        }
    }

    /**
     * Check if there are more albums to fetch for an artist.
     */
    suspend fun hasMoreToFetch(artistId: String): Boolean {
        val cached = skeletonStore.getAlbumIdsForArtist(artistId).size
        val total = serverTotals.value[artistId] ?: return true // Unknown, assume yes
        return cached < total
    }

    /**
     * Get the server total for an artist (if known).
     */
    fun getServerTotal(artistId: String): Int? = serverTotals.value[artistId]

    private fun setServerTotal(artistId: String, total: Int) {
        serverTotals.value = serverTotals.value + (artistId to total)
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
