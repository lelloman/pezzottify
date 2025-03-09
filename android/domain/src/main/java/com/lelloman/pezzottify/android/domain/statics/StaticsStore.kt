package com.lelloman.pezzottify.android.domain.statics

interface StaticsStore {

    fun getArtist(artistId: String): StaticsItemFlow<Artist>

    fun getTrack(trackId: String): StaticsItemFlow<Track>

    fun getAlbum(albumId: String): StaticsItemFlow<Album>

    fun getDiscography(artistId: String): StaticsItemFlow<ArtistDiscography>

    suspend fun storeArtist(artist: Artist): Result<Unit>

    suspend fun storeTrack(track: Track): Result<Unit>

    suspend fun storeAlbum(album: Album): Result<Unit>

    suspend fun storeDiscography(artistDiscography: ArtistDiscography): Result<Unit>

}