package com.lelloman.pezzottify.android.domain.statics.usecase

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.statics.FeaturedAlbum
import com.lelloman.pezzottify.android.domain.statics.FeaturedAlbums
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import javax.inject.Inject

class GetFeaturedAlbums @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
) : UseCase() {

    suspend operator fun invoke(limit: Int = DEFAULT_LIMIT): Result<FeaturedAlbums> =
        when (val response = remoteApiClient.getFeaturedAlbums(limit)) {
            is RemoteApiResponse.Success -> Result.success(
                FeaturedAlbums(
                    heroIndex = response.data.heroIndex,
                    albums = response.data.albums.map { album ->
                        FeaturedAlbum(
                            id = album.id,
                            name = album.name,
                            artistNames = album.artistNames,
                        )
                    },
                )
            )

            is RemoteApiResponse.Error -> Result.failure(
                Throwable("Failed to fetch featured albums")
            )
        }

    companion object {
        const val DEFAULT_LIMIT = 20
    }
}
