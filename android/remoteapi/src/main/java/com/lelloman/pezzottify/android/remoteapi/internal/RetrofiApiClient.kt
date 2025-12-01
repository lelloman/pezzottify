package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.remoteapi.internal.requests.ListeningEventRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.ListeningEventResponse
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
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.Header
import retrofit2.http.POST
import retrofit2.http.Path

internal interface RetrofitApiClient {

    @POST("/v1/auth/login")
    suspend fun login(@Body request: LoginRequest): Response<LoginSuccessResponse>

    @GET("/v1/auth/logout")
    suspend fun logout(@Header("Authorization") authToken: String): Response<Unit>

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

    @GET("/v1/content/album/{albumId}/resolved")
    suspend fun getAlbum(
        @Header("Authorization") authToken: String,
        @Path("albumId") albumId: String
    ): Response<AlbumResponse>

    @GET("/v1/content/track/{trackId}/resolved")
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

    @GET("/v1/user/liked/{contentType}")
    suspend fun getLikedContent(
        @Header("Authorization") authToken: String,
        @Path("contentType") contentType: String,
    ): Response<List<String>>

    @POST("/v1/user/liked/{contentId}")
    suspend fun likeContent(
        @Header("Authorization") authToken: String,
        @Path("contentId") contentId: String,
    ): Response<Unit>

    @DELETE("/v1/user/liked/{contentId}")
    suspend fun unlikeContent(
        @Header("Authorization") authToken: String,
        @Path("contentId") contentId: String,
    ): Response<Unit>

    @POST("/v1/user/listening")
    suspend fun recordListeningEvent(
        @Header("Authorization") authToken: String,
        @Body request: ListeningEventRequest,
    ): Response<ListeningEventResponse>
}