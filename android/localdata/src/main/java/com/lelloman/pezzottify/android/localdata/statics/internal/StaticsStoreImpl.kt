package com.lelloman.pezzottify.android.localdata.statics.internal

import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsItemFlow
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.localdata.statics.model.Album
import com.lelloman.pezzottify.android.localdata.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.statics.model.ArtistDiscography
import com.lelloman.pezzottify.android.localdata.statics.model.Track
import com.lelloman.pezzottify.android.localdata.statics.model.quack
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine

private typealias IArtist = com.lelloman.pezzottify.android.domain.statics.Artist
private typealias ITrack = com.lelloman.pezzottify.android.domain.statics.Track
private typealias IAlbum = com.lelloman.pezzottify.android.domain.statics.Album
private typealias IArtistDiscography = com.lelloman.pezzottify.android.domain.statics.ArtistDiscography

internal class StaticsStoreImpl(
    private val db: StaticsDb,
) : StaticsStore {

    private val staticsDao = db.staticsDao()
    private val staticItemFetchStateDao = db.staticItemFetchStateDao()

    private fun <T> Flow<T?>.withFetchState(
        id: String,
        action: (T?, StaticItemFetchStateRecord?) -> StaticsItem<T>
    ) = this.combine(staticItemFetchStateDao.get(id), action)

    override fun getArtist(artistId: String): StaticsItemFlow<Artist> =
        staticsDao
            .getArtist(artistId)
            .combine(staticItemFetchStateDao.get(artistId)) { artist, fetchState ->
                when {
                    artist != null -> StaticsItem.Loaded(
                        artistId,
                        artist
                    )

                    fetchState?.loading == true -> StaticsItem.Loading(
                        artistId
                    )

                    fetchState?.errorReason != null -> StaticsItem.Error(
                        artistId,
                        Throwable(fetchState.errorReason)
                    )

                    else -> StaticsItem.Error(
                        artistId,
                        Throwable("Unknown error")
                    )
                }
            }

    override fun getTrack(trackId: String): StaticsItemFlow<Track> =
        staticsDao
            .getTrack(trackId).withFetchState(trackId) { track, fetchState ->
                when {
                    track != null -> StaticsItem.Loaded(
                        trackId,
                        track
                    )

                    fetchState?.loading == true -> StaticsItem.Loading(
                        trackId
                    )

                    fetchState?.errorReason != null -> StaticsItem.Error(
                        trackId,
                        Throwable(fetchState.errorReason)
                    )

                    else -> StaticsItem.Error(
                        trackId,
                        Throwable("Unknown error")
                    )
                }
            }

    override fun getAlbum(albumId: String): StaticsItemFlow<Album> =
        staticsDao
            .getAlbum(albumId).withFetchState(albumId) { album, fetchState ->
                when {
                    album != null -> StaticsItem.Loaded(
                        albumId,
                        album
                    )

                    fetchState?.loading == true -> StaticsItem.Loading(
                        albumId
                    )

                    fetchState?.errorReason != null -> StaticsItem.Error(
                        albumId,
                        Throwable(fetchState.errorReason)
                    )

                    else -> StaticsItem.Error(
                        albumId,
                        Throwable("Unknown error")
                    )
                }
            }

    override fun getDiscography(artistId: String): StaticsItemFlow<ArtistDiscography> =
        staticsDao
            .getArtistDiscography(artistId)
            .withFetchState(ArtistDiscography.getId(artistId)) { artistDiscography, fetchState ->
                when {
                    artistDiscography != null -> StaticsItem.Loaded(
                        artistId,
                        artistDiscography
                    )

                    fetchState?.loading == true -> StaticsItem.Loading(
                        artistId
                    )

                    fetchState?.errorReason != null -> StaticsItem.Error(
                        artistId,
                        Throwable(fetchState.errorReason)
                    )

                    else -> StaticsItem.Error(
                        artistId,
                        Throwable("Unknown error")
                    )
                }
            }

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