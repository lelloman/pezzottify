package com.lelloman.pezzottify.android.domain.statics

import kotlinx.coroutines.flow.Flow

interface StaticsStore {

    fun getArtist(artistId: String): Flow<Artist?>

    fun getTrack(trackId: String): Flow<Track?>

    fun getAlbum(albumId: String): Flow<Album?>

    suspend fun storeArtist(artist: Artist): Result<Unit>

    suspend fun storeTrack(track: Track): Result<Unit>

    suspend fun storeAlbum(album: Album): Result<Unit>

    suspend fun deleteAll(): Result<Unit>
}