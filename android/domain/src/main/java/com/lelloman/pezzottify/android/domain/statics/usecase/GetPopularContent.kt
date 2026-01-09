package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.PopularAlbum
import com.lelloman.pezzottify.android.domain.statics.PopularArtist
import com.lelloman.pezzottify.android.domain.statics.PopularContent
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import javax.inject.Inject

class GetPopularContent @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
) : UseCase() {

    suspend operator fun invoke(
        albumsLimit: Int = DEFAULT_ALBUMS_LIMIT,
        artistsLimit: Int = DEFAULT_ARTISTS_LIMIT,
    ): Result<PopularContent> {
        return when (val response = remoteApiClient.getPopularContent(albumsLimit, artistsLimit)) {
            is RemoteApiResponse.Success -> Result.success(
                PopularContent(
                    albums = response.data.albums.map { album ->
                        PopularAlbum(
                            id = album.id,
                            name = album.name,
                            artistNames = album.artistNames,
                        )
                    },
                    artists = response.data.artists.map { artist ->
                        PopularArtist(
                            id = artist.id,
                            name = artist.name,
                        )
                    },
                )
            )
            is RemoteApiResponse.Error -> Result.failure(Throwable("Failed to fetch popular content"))
        }
    }

    companion object {
        const val DEFAULT_ALBUMS_LIMIT = 10
        const val DEFAULT_ARTISTS_LIMIT = 10
    }
}
