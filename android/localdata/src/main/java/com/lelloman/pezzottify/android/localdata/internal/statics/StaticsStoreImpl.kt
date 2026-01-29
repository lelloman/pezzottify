package com.lelloman.pezzottify.android.localdata.internal.statics

import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Album
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Track
import com.lelloman.pezzottify.android.localdata.internal.statics.model.quack
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine

private typealias IArtist = com.lelloman.pezzottify.android.domain.statics.Artist
private typealias ITrack = com.lelloman.pezzottify.android.domain.statics.Track
private typealias IAlbum = com.lelloman.pezzottify.android.domain.statics.Album

internal class StaticsStoreImpl(
    private val db: StaticsDb,
    private val dbSizeCalculator: StaticsDbSizeCalculator,
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

    override suspend fun deleteAll(): Result<Unit> =
        try {
            db.clearAllTables()
            Result.success(Unit)
        } catch (throwable: Throwable) {
            Result.failure(throwable)
        }

    override suspend fun countEntries(): Int {
        return staticsDao.countArtists() +
            staticsDao.countAlbums() +
            staticsDao.countTracks()
    }

    override suspend fun trimOldestPercent(percent: Float): Int {
        require(percent in 0f..1f) { "Percent must be between 0 and 1" }

        val artistCount = staticsDao.countArtists()
        val albumCount = staticsDao.countAlbums()
        val trackCount = staticsDao.countTracks()

        var deleted = 0
        deleted += staticsDao.deleteOldestArtists((artistCount * percent).toInt())
        deleted += staticsDao.deleteOldestAlbums((albumCount * percent).toInt())
        deleted += staticsDao.deleteOldestTracks((trackCount * percent).toInt())

        return deleted
    }

    override suspend fun getDatabaseSizeBytes(): Long {
        return dbSizeCalculator.getDatabaseSizeBytes()
    }

    override suspend fun vacuum() {
        db.openHelper.writableDatabase.execSQL("VACUUM")
    }

    override suspend fun deleteAlbum(albumId: String): Result<Unit> = try {
        staticsDao.deleteAlbum(albumId)
        Result.success(Unit)
    } catch (throwable: Throwable) {
        Result.failure(throwable)
    }

    override suspend fun deleteArtist(artistId: String): Result<Unit> = try {
        staticsDao.deleteArtist(artistId)
        Result.success(Unit)
    } catch (throwable: Throwable) {
        Result.failure(throwable)
    }

    override suspend fun deleteTrack(trackId: String): Result<Unit> = try {
        staticsDao.deleteTrack(trackId)
        Result.success(Unit)
    } catch (throwable: Throwable) {
        Result.failure(throwable)
    }
}
