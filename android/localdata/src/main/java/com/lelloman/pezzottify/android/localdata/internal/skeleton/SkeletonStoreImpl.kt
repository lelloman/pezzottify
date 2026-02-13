package com.lelloman.pezzottify.android.localdata.internal.skeleton

import com.lelloman.pezzottify.android.domain.skeleton.AlbumArtistRelationship
import com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject

internal class SkeletonStoreImpl @Inject constructor(
    private val skeletonDao: SkeletonDao
) : SkeletonStore {

    override suspend fun getAlbumIdsForArtist(artistId: String): List<String> =
        withContext(Dispatchers.IO) {
            skeletonDao.getAlbumIdsForArtist(artistId)
        }

    override fun observeAlbumIdsForArtist(artistId: String): kotlinx.coroutines.flow.Flow<List<String>> =
        skeletonDao.observeAlbumIdsForArtist(artistId)

    override fun observeAppearsOnAlbumIdsForArtist(artistId: String): kotlinx.coroutines.flow.Flow<List<String>> =
        skeletonDao.observeAppearsOnAlbumIdsForArtist(artistId)

    override suspend fun getTrackIdsForAlbum(albumId: String): List<String> =
        withContext(Dispatchers.IO) {
            skeletonDao.getTrackIdsForAlbum(albumId)
        }

    override suspend fun insertAlbumArtists(albumArtists: List<AlbumArtistRelationship>) =
        withContext(Dispatchers.IO) {
            if (albumArtists.isEmpty()) return@withContext

            // Extract unique artist IDs and insert them first (to satisfy foreign key)
            val artistIds = albumArtists.map { it.artistId }.distinct()
            artistIds.forEach { artistId ->
                skeletonDao.insertArtist(
                    com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonArtist(artistId)
                )
            }

            // Extract unique album IDs and insert them (to satisfy foreign key)
            val albums = albumArtists.map { it.albumId }.distinct().map { albumId ->
                com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbum(albumId)
            }
            skeletonDao.insertAlbums(albums)

            // Now insert the relationships with order index and appears-on flag
            val daoAlbumArtists = albumArtists.map { relationship ->
                com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbumArtist(
                    artistId = relationship.artistId,
                    albumId = relationship.albumId,
                    orderIndex = relationship.orderIndex,
                    isAppearsOn = relationship.isAppearsOn
                )
            }
            skeletonDao.insertAlbumArtists(daoAlbumArtists)
        }

    override suspend fun deleteAlbumsForArtist(artistId: String): Result<Unit> =
        withContext(Dispatchers.IO) {
            runCatching {
                skeletonDao.deleteAlbumsForArtist(artistId)
            }
        }

    override suspend fun deleteAppearsOnAlbumsForArtist(artistId: String): Result<Unit> =
        withContext(Dispatchers.IO) {
            runCatching {
                skeletonDao.deleteAppearsOnAlbumsForArtist(artistId)
            }
        }

    override suspend fun clear(): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            skeletonDao.deleteAllTracks()
            skeletonDao.deleteAllAlbumArtists()
            skeletonDao.deleteAllAlbums()
            skeletonDao.deleteAllArtists()
            skeletonDao.deleteAllMeta()
        }
    }
}
