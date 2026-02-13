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
 *
 * All methods accept an optional isAppearsOn parameter to fetch "appears on"
 * albums separately from primary discography.
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

    private fun stateKey(artistId: String, isAppearsOn: Boolean): String =
        if (isAppearsOn) "$artistId:ao" else artistId

    // Track server totals per artist (so we know if there's more to fetch)
    private val serverTotals = MutableStateFlow<Map<String, Int>>(emptyMap())

    // Track current offset per artist (how many albums we've fetched from server)
    private val currentOffsets = MutableStateFlow<Map<String, Int>>(emptyMap())

    // Track loading state per artist
    private val loadingStates = MutableStateFlow<Map<String, Boolean>>(emptyMap())

    // Track errors per artist
    private val errors = MutableStateFlow<Map<String, String?>>(emptyMap())

    // Mutex per key to prevent concurrent fetches
    private val fetchMutexes = mutableMapOf<String, Mutex>()

    private fun getMutex(key: String): Mutex {
        return synchronized(fetchMutexes) {
            fetchMutexes.getOrPut(key) { Mutex() }
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
    fun observeDiscography(artistId: String, isAppearsOn: Boolean = false): Flow<DiscographyState> {
        val key = stateKey(artistId, isAppearsOn)
        val albumIdsFlow = if (isAppearsOn) {
            skeletonStore.observeAppearsOnAlbumIdsForArtist(artistId)
        } else {
            skeletonStore.observeAlbumIdsForArtist(artistId)
        }
        return combine(
            albumIdsFlow,
            serverTotals,
            currentOffsets,
            loadingStates,
            errors
        ) { albumIds, totals, offsets, loading, errs ->
            val total = totals[key]
            val offset = offsets[key] ?: 0
            DiscographyState(
                albumIds = albumIds,
                totalOnServer = total,
                isLoading = loading[key] == true,
                hasMore = total != null && offset < total,
                error = errs[key]
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
    suspend fun fetchFirstPage(artistId: String, isAppearsOn: Boolean = false): FetchPageResult = withContext(Dispatchers.IO) {
        val key = stateKey(artistId, isAppearsOn)
        val mutex = getMutex(key)

        // Prevent concurrent fetches for same artist
        if (!mutex.tryLock()) {
            logger.debug("fetchFirstPage($key) already in progress, skipping")
            return@withContext FetchPageResult.Success(0, true)
        }

        try {
            setLoading(key, true)
            clearError(key)

            logger.info("fetchFirstPage($key) pageSize=$PAGE_SIZE")

            // Fetch first page from server
            when (val response = remoteApiClient.getArtistDiscography(
                artistId = artistId,
                offset = 0,
                limit = PAGE_SIZE,
                appearsOn = isAppearsOn
            )) {
                is RemoteApiResponse.Success -> {
                    val data = response.data

                    // Update server total and reset offset
                    setServerTotal(key, data.total)
                    setCurrentOffset(key, data.albums.size)

                    // Clear existing albums and insert fresh data with order index
                    if (isAppearsOn) {
                        skeletonStore.deleteAppearsOnAlbumsForArtist(artistId)
                    } else {
                        skeletonStore.deleteAlbumsForArtist(artistId)
                    }

                    val albumArtists = data.albums.mapIndexed { index, album ->
                        AlbumArtistRelationship(
                            artistId = artistId,
                            albumId = album.id,
                            orderIndex = index,
                            isAppearsOn = isAppearsOn
                        )
                    }
                    skeletonStore.insertAlbumArtists(albumArtists)

                    val hasMore = data.albums.size < data.total

                    logger.info("fetchFirstPage($key) fetched ${data.albums.size} albums, total=${data.total}, hasMore=$hasMore")

                    FetchPageResult.Success(
                        fetchedCount = data.albums.size,
                        hasMore = hasMore
                    )
                }
                is RemoteApiResponse.Error -> {
                    val errorMsg = "Failed to fetch discography: $response"
                    logger.error("fetchFirstPage($key) $errorMsg")
                    setError(key, errorMsg)
                    FetchPageResult.Error(errorMsg)
                }
            }
        } finally {
            setLoading(key, false)
            mutex.unlock()
        }
    }

    /**
     * Fetch more albums for an artist (next page).
     *
     * Call this when the user scrolls down to load more albums.
     * Albums are appended to the existing cache with correct order indices.
     */
    suspend fun fetchMoreAlbums(artistId: String, isAppearsOn: Boolean = false): FetchPageResult = withContext(Dispatchers.IO) {
        val key = stateKey(artistId, isAppearsOn)
        val mutex = getMutex(key)

        // Prevent concurrent fetches for same artist
        if (!mutex.tryLock()) {
            logger.debug("fetchMoreAlbums($key) already in progress, skipping")
            return@withContext FetchPageResult.Success(0, true)
        }

        try {
            setLoading(key, true)
            clearError(key)

            val offset = currentOffsets.value[key] ?: 0
            val total = serverTotals.value[key]

            // Check if we already have all albums
            if (total != null && offset >= total) {
                logger.info("fetchMoreAlbums($key) already complete: offset=$offset >= total=$total")
                return@withContext FetchPageResult.AlreadyComplete
            }

            logger.info("fetchMoreAlbums($key) offset=$offset, pageSize=$PAGE_SIZE")

            // Fetch next page from server
            when (val response = remoteApiClient.getArtistDiscography(
                artistId = artistId,
                offset = offset,
                limit = PAGE_SIZE,
                appearsOn = isAppearsOn
            )) {
                is RemoteApiResponse.Success -> {
                    val data = response.data

                    // Update server total and offset
                    setServerTotal(key, data.total)
                    val newOffset = offset + data.albums.size
                    setCurrentOffset(key, newOffset)

                    // Append albums with correct order index
                    val albumArtists = data.albums.mapIndexed { index, album ->
                        AlbumArtistRelationship(
                            artistId = artistId,
                            albumId = album.id,
                            orderIndex = offset + index,
                            isAppearsOn = isAppearsOn
                        )
                    }
                    skeletonStore.insertAlbumArtists(albumArtists)

                    val hasMore = newOffset < data.total

                    logger.info("fetchMoreAlbums($key) fetched ${data.albums.size} albums, offset=$newOffset/${data.total}, hasMore=$hasMore")

                    FetchPageResult.Success(
                        fetchedCount = data.albums.size,
                        hasMore = hasMore
                    )
                }
                is RemoteApiResponse.Error -> {
                    val errorMsg = "Failed to fetch more albums: $response"
                    logger.error("fetchMoreAlbums($key) $errorMsg")
                    setError(key, errorMsg)
                    FetchPageResult.Error(errorMsg)
                }
            }
        } finally {
            setLoading(key, false)
            mutex.unlock()
        }
    }

    private fun setServerTotal(key: String, total: Int) {
        serverTotals.value = serverTotals.value + (key to total)
    }

    private fun setCurrentOffset(key: String, offset: Int) {
        currentOffsets.value = currentOffsets.value + (key to offset)
    }

    private fun setLoading(key: String, loading: Boolean) {
        loadingStates.value = if (loading) {
            loadingStates.value + (key to true)
        } else {
            loadingStates.value - key
        }
    }

    private fun setError(key: String, error: String) {
        errors.value = errors.value + (key to error)
    }

    private fun clearError(key: String) {
        errors.value = errors.value - key
    }
}
