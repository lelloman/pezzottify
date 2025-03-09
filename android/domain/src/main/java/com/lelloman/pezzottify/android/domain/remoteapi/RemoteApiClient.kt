package com.lelloman.pezzottify.android.domain.remoteapi

import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable


interface RemoteApiClient {

    suspend fun login(userHandle: String, password: String): RemoteApiResponse<LoginSuccessResponse>

    suspend fun getArtist(artistId: String): RemoteApiResponse<ArtistResponse>

    suspend fun getArtistDiscography(artistId: String): RemoteApiResponse<ArtistDiscographyResponse>

    suspend fun getAlbum(albumId: String): RemoteApiResponse<AlbumResponse>

    suspend fun getTrack(trackId: String): RemoteApiResponse<TrackResponse>

    suspend fun getImage(imageId: String): RemoteApiResponse<ImageResponse>

    suspend fun search(
        query: String,
        filters: List<SearchFilter>? = null
    ): RemoteApiResponse<SearchResponse>

    @Serializable
    enum class SearchFilter {
        Album,
        Artist,
        Track,
    }

    interface HostUrlProvider {
        val hostUrl: StateFlow<String>
    }
}