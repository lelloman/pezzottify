package com.lelloman.pezzottify.android.domain.skeleton

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SkeletonChangeDto
import com.lelloman.pezzottify.android.domain.remoteapi.response.SkeletonDeltaResponse
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Handles synchronization of the catalog skeleton (artist/album/track IDs and relationships)
 * from the server to local storage.
 *
 * Uses delta sync when possible for efficiency, falling back to full sync when needed.
 */
@Singleton
class CatalogSkeletonSyncer @Inject constructor(
    private val api: RemoteApiClient,
    private val skeletonStore: SkeletonStore,
    loggerFactory: LoggerFactory
) {
    private val logger: Logger by loggerFactory

    /**
     * Result of a sync operation.
     */
    sealed class SyncResult {
        object Success : SyncResult()
        object AlreadyUpToDate : SyncResult()
        data class Failed(val error: String) : SyncResult()
    }

    /**
     * Perform skeleton sync.
     * - If no local version exists, performs a full sync.
     * - Otherwise attempts delta sync, falling back to full sync if needed.
     */
    suspend fun sync(): SyncResult {
        val localVersion = skeletonStore.getVersion() ?: 0L

        if (localVersion == 0L) {
            logger.info("sync() no local version, performing full sync")
            return fullSync()
        }

        logger.info("sync() local version $localVersion, attempting delta sync")

        return when (val response = api.getSkeletonDelta(localVersion)) {
            is RemoteApiResponse.Success -> {
                if (response.data.changes.isEmpty()) {
                    logger.info("sync() already up to date at version ${response.data.toVersion}")
                    SyncResult.AlreadyUpToDate
                } else {
                    applyDelta(response.data)
                }
            }
            is RemoteApiResponse.Error.NotFound -> {
                logger.warn("sync() version $localVersion too old, performing full sync")
                fullSync()
            }
            is RemoteApiResponse.Error -> {
                logger.error("sync() delta sync failed: $response")
                SyncResult.Failed(response.toString())
            }
        }
    }

    /**
     * Force a full sync, ignoring local version.
     */
    suspend fun forceFullSync(): SyncResult = fullSync()

    /**
     * Verify that local checksum matches server.
     * Returns false if checksums don't match or verification fails.
     */
    suspend fun verifyChecksum(): Boolean {
        val localChecksum = skeletonStore.getChecksum() ?: return false

        return when (val response = api.getSkeletonVersion()) {
            is RemoteApiResponse.Success -> {
                val match = response.data.checksum == localChecksum
                if (!match) {
                    logger.warn("verifyChecksum() mismatch! Local: $localChecksum, Remote: ${response.data.checksum}")
                }
                match
            }
            else -> {
                logger.error("verifyChecksum() failed to get remote version: $response")
                false
            }
        }
    }

    private suspend fun fullSync(): SyncResult {
        return when (val response = api.getFullSkeleton()) {
            is RemoteApiResponse.Success -> {
                val data = response.data
                logger.info("fullSync() received - ${data.artists.size} artists, ${data.albums.size} albums, ${data.tracks.size} tracks")

                val fullSkeleton = FullSkeleton(
                    version = data.version,
                    checksum = data.checksum,
                    artists = data.artists,
                    albums = data.albums.map { SkeletonAlbumData(it.id, it.artistIds) },
                    tracks = data.tracks.map { SkeletonTrackData(it.id, it.albumId) }
                )

                skeletonStore.replaceAll(fullSkeleton).fold(
                    onSuccess = {
                        logger.info("fullSync() complete at version ${data.version}")
                        SyncResult.Success
                    },
                    onFailure = { e ->
                        logger.error("fullSync() failed to store skeleton: ${e.message}")
                        SyncResult.Failed(e.message ?: "Unknown storage error")
                    }
                )
            }
            is RemoteApiResponse.Error -> {
                logger.error("fullSync() failed: $response")
                SyncResult.Failed(response.toString())
            }
        }
    }

    private suspend fun applyDelta(deltaResponse: SkeletonDeltaResponse): SyncResult {
        logger.info("applyDelta() applying ${deltaResponse.changes.size} changes (${deltaResponse.fromVersion} -> ${deltaResponse.toVersion})")

        val changes = deltaResponse.changes.map { it.toDomainChange() }

        val delta = SkeletonDelta(
            fromVersion = deltaResponse.fromVersion,
            toVersion = deltaResponse.toVersion,
            checksum = deltaResponse.checksum,
            changes = changes
        )

        return skeletonStore.applyDelta(delta).fold(
            onSuccess = {
                logger.info("applyDelta() complete, now at version ${deltaResponse.toVersion}")
                SyncResult.Success
            },
            onFailure = { e ->
                logger.error("applyDelta() failed: ${e.message}")
                SyncResult.Failed(e.message ?: "Unknown storage error")
            }
        )
    }

    private fun SkeletonChangeDto.toDomainChange(): SkeletonChange {
        return when (type) {
            "artist_added" -> SkeletonChange.ArtistAdded(id)
            "artist_removed" -> SkeletonChange.ArtistRemoved(id)
            "album_added" -> SkeletonChange.AlbumAdded(id, artistIds ?: emptyList())
            "album_removed" -> SkeletonChange.AlbumRemoved(id)
            "track_added" -> SkeletonChange.TrackAdded(id, albumId ?: "")
            "track_removed" -> SkeletonChange.TrackRemoved(id)
            else -> throw IllegalArgumentException("Unknown skeleton change type: $type")
        }
    }
}
