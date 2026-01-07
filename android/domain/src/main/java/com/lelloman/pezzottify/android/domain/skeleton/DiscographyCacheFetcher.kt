package com.lelloman.pezzottify.android.domain.skeleton

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.skeleton.AlbumArtistRelationship
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.delay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * On-demand discography cache fetcher.
 *
 * Fetches artist's discography from server in batches when needed,
 * storing results in skeleton cache. UI observes skeleton
 * for progressive loading as albums are cached.
 */
@Singleton
class DiscographyCacheFetcher @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val skeletonStore: SkeletonStore,
    loggerFactory: LoggerFactory,
) {

    private val logger: Logger by loggerFactory

    companion object {
        const val BATCH_SIZE = 20
        const val BATCH_DELAY_MS = 500L
        const val MAX_ALBUMS_TO_FETCH = 500
    }

    /**
     * Ensure discography is cached for an artist.
     *
     * Logic:
     * 1. Check how many albums are cached in skeleton
     * 2. Query server for total album count (lightweight, limit=0)
     * 3. If cached >= total, done (no fetch needed)
     * 4. If cached < total, fetch missing albums in batches
     *
     * Returns number of albums cached after fetch (or existing cache).
     */
    suspend fun ensureDiscographyCached(artistId: String): Int = withContext(Dispatchers.IO) {
        // Step 1: Check cached count
        val cachedAlbums = skeletonStore.getAlbumIdsForArtist(artistId)
        val cachedCount = cachedAlbums.size

        logger.info("ensureDiscographyCached($artistId) cachedCount=$cachedCount")

        // Step 2: Get server total (lightweight query)
        val serverTotal = when (val response = remoteApiClient.getArtistDiscography(
            artistId = artistId,
            limit = 0 // Just get metadata, no albums
        )) {
            is RemoteApiResponse.Success -> {
                response.data.total
            }
            is RemoteApiResponse.Error -> {
                logger.warn("Failed to get server total for artist $artistId: $response")
                cachedCount // Return cached count
            }
        }

        logger.info("ensureDiscographyCached($artistId) serverTotal=$serverTotal")

        // Step 3: Decide if fetch is needed
        if (cachedCount >= serverTotal) {
            logger.info("ensureDiscographyCached($artistId) already have all $cachedCount albums, no fetch needed")
            return cachedCount
        }

        // Step 4: Fetch missing albums in batches
        logger.info("ensureDiscographyCached($artistId) fetching $serverTotal albums (have $cachedCount, need ${serverTotal - cachedCount})")

        var offset = cachedCount // Start from what we already have
        var totalFetched = cachedCount
        var pageCount = 0

        while (offset < serverTotal && totalFetched < MAX_ALBUMS_TO_FETCH) {
            // Fetch batch
            val response = remoteApiClient.getArtistDiscography(
                artistId = artistId,
                offset = offset,
                limit = BATCH_SIZE
            )

            when (response) {
                is RemoteApiResponse.Success -> {
                    val albumArtists = response.data.albums.map { album ->
                        AlbumArtistRelationship(
                            artistId = artistId,
                            albumId = album.id
                        )
                    }

                    // Cache immediately (UI updates via Flow)
                    skeletonStore.insertAlbumArtists(albumArtists)

                    val fetched = albumArtists.size
                    totalFetched += fetched
                    offset += fetched
                    pageCount++

                    logger.debug("ensureDiscographyCached($artistId) fetched page $pageCount: $fetched albums, total=$totalFetched/$serverTotal")
                }
                is RemoteApiResponse.Error -> {
                    logger.error("ensureDiscographyCached($artistId) failed to fetch page: $response")
                    break
                }
            }

            // Throttle between batches (network friendliness)
            if (offset < serverTotal && totalFetched < MAX_ALBUMS_TO_FETCH) {
                delay(BATCH_DELAY_MS)
            }
        }

        val result = when {
            totalFetched < serverTotal -> {
                logger.info("ensureDiscographyCached($artistId) stopped at $totalFetched albums (server total: $serverTotal, max: $MAX_ALBUMS_TO_FETCH)")
                "stopped"
            }
            totalFetched == serverTotal -> {
                logger.info("ensureDiscographyCached($artistId) completed: $totalFetched albums cached")
                "completed"
            }
            else -> {
                "unknown"
            }
        }
        return totalFetched
    }
}

    /**
     * Ensure discography is cached for an artist.
     *
     * Logic:
     * 1. Check how many albums are cached in skeleton
     * 2. Query server for total album count (lightweight, limit=0)
     * 3. If cached >= total, done (no fetch needed)
     * 4. If cached < total, fetch missing albums in batches
     *
     * Returns number of albums cached after fetch (or existing cache).
     */
    suspend fun ensureDiscographyCached(artistId: String): Int = withContext(Dispatchers.IO) {
        // Step 1: Check cached count
        val cachedAlbums = skeletonStore.getAlbumIdsForArtist(artistId)
        val cachedCount = cachedAlbums.size

        logger.info("ensureDiscographyCached($artistId) cachedCount=$cachedCount")

        // Step 2: Get server total (lightweight query)
        val serverTotal = when (val response = remoteApiClient.getArtistDiscography(
            artistId = artistId,
            limit = 0 // Just get metadata, no albums
        )) {
            is com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiResponse.Success -> {
                response.data.total
            }
            is com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiResponse.Error -> {
                logger.warn("Failed to get server total for artist $artistId: $response")
                cachedCount // Return cached count
            }
        }

        logger.info("ensureDiscographyCached($artistId) serverTotal=$serverTotal")

        // Step 3: Decide if fetch is needed
        if (cachedCount >= serverTotal) {
            logger.info("ensureDiscographyCached($artistId) already have all $cachedCount albums, no fetch needed")
            return cachedCount
        }

        // Step 4: Fetch missing albums in batches
        logger.info("ensureDiscographyCached($artistId) fetching $serverTotal albums (have $cachedCount, need ${serverTotal - cachedCount})")

        var offset = cachedCount // Start from what we already have
        var totalFetched = cachedCount
        var pageCount = 0

        while (offset < serverTotal && totalFetched < MAX_ALBUMS_TO_FETCH) {
            // Fetch batch
            val response = remoteApiClient.getArtistDiscography(
                artistId = artistId,
                offset = offset,
                limit = BATCH_SIZE
            )

            when (response) {
                is com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiResponse.Success -> {
                    val albumArtists = response.data.albums.map { album ->
                        AlbumArtistRelationship(
                            artistId = artistId,
                            albumId = album.id
                        )
                    }

                    // Cache immediately (UI updates via Flow)
                    skeletonStore.insertAlbumArtists(albumArtists)

                    val fetched = albumArtists.size
                    totalFetched += fetched
                    offset += fetched
                    pageCount++

                    logger.debug("ensureDiscographyCached($artistId) fetched page $pageCount: $fetched albums, total=$totalFetched/$serverTotal")
                }
                is com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiResponse.Error -> {
                    logger.error("ensureDiscographyCached($artistId) failed to fetch page: $response")
                    break
                }
            }

            // Throttle between batches (network friendliness)
            if (offset < serverTotal && totalFetched < MAX_ALBUMS_TO_FETCH) {
                delay(BATCH_DELAY_MS)
            }
        }

        if (totalFetched < serverTotal) {
            logger.info("ensureDiscographyCached($artistId) stopped at $totalFetched albums (server total: $serverTotal, max: $MAX_ALBUMS_TO_FETCH)")
        } else {
            logger.info("ensureDiscographyCached($artistId) completed: $totalFetched albums cached")
        }

        return totalFetched
    }
}
