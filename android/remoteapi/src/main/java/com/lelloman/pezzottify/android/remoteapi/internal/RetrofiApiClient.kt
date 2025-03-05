package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.remoteapi.internal.requests.LoginRequest
import com.lelloman.pezzottify.android.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.remoteapi.response.LoginSuccessResponse
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
}