package com.lelloman.pezzottify.android.localdata.statics.internal

import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.localdata.statics.model.Album
import com.lelloman.pezzottify.android.localdata.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.statics.model.ArtistDiscography
import com.lelloman.pezzottify.android.localdata.statics.model.Track
import com.lelloman.pezzottify.android.localdata.statics.model.quack
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine

private typealias IArtist = com.lelloman.pezzottify.android.domain.statics.Artist
private typealias ITrack = com.lelloman.pezzottify.android.domain.statics.Track
private typealias IAlbum = com.lelloman.pezzottify.android.domain.statics.Album
private typealias IArtistDiscography = com.lelloman.pezzottify.android.domain.statics.ArtistDiscography

internal class StaticsStoreImpl(
    private val db: StaticsDb,
    loggerFactory: LoggerFactory,
) : StaticsStore {

    private val logger by loggerFactory
    private val staticsDao = db.staticsDao()
    private val staticItemFetchStateDao = db.staticItemFetchStateDao()

    private fun <T> Flow<T?>.withFetchState(
        id: String,
        action: (T?, StaticItemFetchStateRecord?) -> StaticsItem<T>
    ) = this.combine(staticItemFetchStateDao.get(id), action)

    override fun getArtist(artistId: String): Flow<Artist?> = staticsDao.getArtist(artistId)

    override fun getTrack(trackId: String): Flow<Track?> = staticsDao.getTrack(trackId)

    override fun getAlbum(albumId: String): Flow<Album?> = staticsDao.getAlbum(albumId)

    override fun getDiscography(artistId: String): Flow<ArtistDiscography?> =
        staticsDao.getArtistDiscography(artistId)

    override suspend fun storeArtist(artist: IArtist): Result<Unit> = try {
        staticsDao.insertArtist(artist.quack())
        Result.success(Unit)
    } catch (throwable: Throwable) {
        Result.failure(throwable)
    }

    override suspend fun storeTrack(track: ITrack): Result<Unit> = try {
        staticsDao.insertTrack(track.quack())
        Result.success(Unit)
    } catch (throwable: Throwable) {
        Result.failure(throwable)
    }

    override suspend fun storeAlbum(album: IAlbum): Result<Unit> = try {
        staticsDao.insertAlbum(album.quack())
        Result.success(Unit)
    } catch (throwable: Throwable) {
        logger.error("Error while storing album: $album", throwable)
        Result.failure(throwable)
    }

    override suspend fun storeDiscography(artistDiscography: IArtistDiscography): Result<Unit> =
        try {
            staticsDao.insertArtistDiscography(artistDiscography.quack())
            Result.success(Unit)
        } catch (throwable: Throwable) {
            Result.failure(throwable)
        }

    override suspend fun deleteAll(): Result<Unit> =
        try {
            db.clearAllTables()
            Result.success(Unit)
        } catch (throwable: Throwable) {
            Result.failure(throwable)
        }
}