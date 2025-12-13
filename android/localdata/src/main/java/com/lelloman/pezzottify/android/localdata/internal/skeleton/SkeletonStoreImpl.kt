package com.lelloman.pezzottify.android.localdata.internal.skeleton

import com.lelloman.pezzottify.android.domain.skeleton.FullSkeleton
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonChange
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonCounts
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonDelta
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbum
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbumArtist
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonArtist
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonMeta
import com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonTrack
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject

internal class SkeletonStoreImpl @Inject constructor(
    private val skeletonDao: SkeletonDao
) : SkeletonStore {

    override suspend fun getVersion(): Long? = withContext(Dispatchers.IO) {
        skeletonDao.getVersion()?.toLongOrNull()
    }

    override suspend fun getChecksum(): String? = withContext(Dispatchers.IO) {
        skeletonDao.getChecksum()
    }

    override suspend fun getAlbumIdsForArtist(artistId: String): List<String> =
        withContext(Dispatchers.IO) {
            skeletonDao.getAlbumIdsForArtist(artistId)
        }

    override fun observeAlbumIdsForArtist(artistId: String): kotlinx.coroutines.flow.Flow<List<String>> =
        skeletonDao.observeAlbumIdsForArtist(artistId)

    override suspend fun getTrackIdsForAlbum(albumId: String): List<String> =
        withContext(Dispatchers.IO) {
            skeletonDao.getTrackIdsForAlbum(albumId)
        }

    override suspend fun getCounts(): SkeletonCounts = withContext(Dispatchers.IO) {
        SkeletonCounts(
            artists = skeletonDao.getArtistCount(),
            albums = skeletonDao.getAlbumCount(),
            tracks = skeletonDao.getTrackCount()
        )
    }

    override suspend fun replaceAll(fullSkeleton: FullSkeleton): Result<Unit> =
        withContext(Dispatchers.IO) {
            runCatching {
                val artists = fullSkeleton.artists.map { SkeletonArtist(it) }
                val albums = fullSkeleton.albums.map { SkeletonAlbum(it.id) }
                val albumArtists = fullSkeleton.albums.flatMap { album ->
                    album.artistIds.map { artistId ->
                        SkeletonAlbumArtist(album.id, artistId)
                    }
                }
                val tracks = fullSkeleton.tracks.map { SkeletonTrack(it.id, it.albumId) }

                skeletonDao.replaceAll(
                    artists = artists,
                    albums = albums,
                    albumArtists = albumArtists,
                    tracks = tracks,
                    version = fullSkeleton.version.toString(),
                    checksum = fullSkeleton.checksum
                )
            }
        }

    override suspend fun applyDelta(delta: SkeletonDelta): Result<Unit> =
        withContext(Dispatchers.IO) {
            runCatching {
                // Parse changes into categorized lists
                val addedArtists = mutableListOf<SkeletonArtist>()
                val removedArtistIds = mutableListOf<String>()
                val addedAlbums = mutableListOf<SkeletonAlbum>()
                val addedAlbumArtists = mutableListOf<SkeletonAlbumArtist>()
                val removedAlbumIds = mutableListOf<String>()
                val addedTracks = mutableListOf<SkeletonTrack>()
                val removedTrackIds = mutableListOf<String>()

                delta.changes.forEach { change ->
                    when (change) {
                        is SkeletonChange.ArtistAdded -> {
                            addedArtists.add(SkeletonArtist(change.artistId))
                        }
                        is SkeletonChange.ArtistRemoved -> {
                            removedArtistIds.add(change.artistId)
                        }
                        is SkeletonChange.AlbumAdded -> {
                            addedAlbums.add(SkeletonAlbum(change.albumId))
                            change.artistIds.forEach { artistId ->
                                addedAlbumArtists.add(
                                    SkeletonAlbumArtist(change.albumId, artistId)
                                )
                            }
                        }
                        is SkeletonChange.AlbumRemoved -> {
                            removedAlbumIds.add(change.albumId)
                        }
                        is SkeletonChange.TrackAdded -> {
                            addedTracks.add(SkeletonTrack(change.trackId, change.albumId))
                        }
                        is SkeletonChange.TrackRemoved -> {
                            removedTrackIds.add(change.trackId)
                        }
                    }
                }

                skeletonDao.applyDelta(
                    addedArtists = addedArtists,
                    removedArtistIds = removedArtistIds,
                    addedAlbums = addedAlbums,
                    addedAlbumArtists = addedAlbumArtists,
                    removedAlbumIds = removedAlbumIds,
                    addedTracks = addedTracks,
                    removedTrackIds = removedTrackIds,
                    newVersion = delta.toVersion.toString(),
                    newChecksum = delta.checksum
                )
            }
        }

    override suspend fun clear(): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            skeletonDao.deleteAllTracks()
            skeletonDao.deleteAllAlbumArtists()
            skeletonDao.deleteAllAlbums()
            skeletonDao.deleteAllArtists()
            skeletonDao.setMeta(SkeletonMeta(SkeletonMeta.KEY_VERSION, "0"))
            skeletonDao.setMeta(SkeletonMeta(SkeletonMeta.KEY_CHECKSUM, ""))
        }
    }
}
