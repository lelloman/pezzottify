package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.remoteapi.internal.requests.LoginRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.SearchRequest
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import okhttp3.ResponseBody
import retrofit2.Response
import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.Header
import retrofit2.http.POST
import retrofit2.http.Path

internal interface RetrofitApiClient {

    @POST("/v1/auth/login")
    suspend fun login(@Body request: LoginRequest): Response<LoginSuccessResponse>

    @GET("/v1/content/artist/{artistId}")
    suspend fun getArtist(
        @Header("Authorization") authToken: String,
        @Path("artistId") artistId: String
    ): Response<ArtistResponse>

    @GET("/v1/content/artist/{artistId}/discography")
    suspend fun getArtistDiscography(
        @Header("Authorization") authToken: String,
        @Path("artistId") artistId: String
    ): Response<ArtistDiscographyResponse>

    @GET("/v1/content/album/{albumId}")
    suspend fun getAlbum(
        @Header("Authorization") authToken: String,
        @Path("albumId") albumId: String
    ): Response<AlbumResponse>

    @GET("/v1/content/track/{trackId}")
    suspend fun getTrack(
        @Header("Authorization") authToken: String,
        @Path("trackId") trackId: String
    ): Response<TrackResponse>

    @GET("/v1/content/image/{imageId}")
    suspend fun getImage(
        @Header("Authorization") authToken: String,
        @Path("imageId") imageId: String
    ): Response<ResponseBody>

    @POST("/v1/content/search")
    suspend fun search(
        @Header("Authorization") authToken: String,
        @Body request: SearchRequest,
    ): Response<SearchResponse>
}