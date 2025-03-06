package com.lelloman.pezzottify.android.localdata.statics

import com.lelloman.pezzottify.android.localdata.statics.model.Album
import com.lelloman.pezzottify.android.localdata.statics.model.Artist
import com.lelloman.pezzottify.android.localdata.statics.model.ArtistDiscography
import com.lelloman.pezzottify.android.localdata.statics.model.Track

interface StaticsStore {

    fun getArtist(artistId: String): StaticsItemFlow<Artist>

    fun getTrack(trackId: String): StaticsItemFlow<Track>

    fun getAlbum(albumId: String): StaticsItemFlow<Album>

    fun getDiscography(artistId: String): StaticsItemFlow<ArtistDiscography>

    suspend fun storeArtist(artist: Artist): Result<Void>

    suspend fun storeTrack(track: Track): Result<Void>

    suspend fun storeAlbum(album: Album): Result<Void>

    suspend fun storeDiscography(artistDiscography: ArtistDiscography): Result<Void>
}