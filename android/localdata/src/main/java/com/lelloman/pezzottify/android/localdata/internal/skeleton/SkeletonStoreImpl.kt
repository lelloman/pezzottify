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

    override suspend fun getTrackIdsForAlbum(albumId: String): List<String> =
        withContext(Dispatchers.IO) {
            skeletonDao.getTrackIdsForAlbum(albumId)
        }

    override suspend fun insertAlbumArtists(albumArtists: List<AlbumArtistRelationship>) =
        withContext(Dispatchers.IO) {
            val daoAlbumArtists = albumArtists.map { relationship ->
                com.lelloman.pezzottify.android.localdata.internal.skeleton.model.SkeletonAlbumArtist(
                    artistId = relationship.artistId,
                    albumId = relationship.albumId
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
