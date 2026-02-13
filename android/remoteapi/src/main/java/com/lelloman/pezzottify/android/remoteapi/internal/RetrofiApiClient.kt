package com.lelloman.pezzottify.android.remoteapi.internal

import com.lelloman.pezzottify.android.remoteapi.internal.requests.AddTracksToPlaylistRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.BatchContentRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.BatchContentResponse
import com.lelloman.pezzottify.android.remoteapi.internal.requests.CreatePlaylistRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.CreatePlaylistResponse
import com.lelloman.pezzottify.android.remoteapi.internal.requests.ImpressionRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.ListeningEventRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.RemoveTracksFromPlaylistRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.ListeningEventResponse
import com.lelloman.pezzottify.android.remoteapi.internal.requests.LoginRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.RequestAlbumDownloadBody
import com.lelloman.pezzottify.android.remoteapi.internal.requests.SearchRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.SubmitBugReportRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.UpdatePlaylistRequest
import com.lelloman.pezzottify.android.remoteapi.internal.requests.UpdateUserSettingsRequest
import com.lelloman.pezzottify.android.domain.remoteapi.request.DeviceSharePolicyRequest
import com.lelloman.pezzottify.android.domain.remoteapi.response.DevicesResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.DeviceSharePolicy
import com.lelloman.pezzottify.android.domain.remoteapi.SubmitBugReportResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.CatalogStatsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadLimitsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ListeningEventItem
import com.lelloman.pezzottify.android.domain.remoteapi.response.MyDownloadRequestsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RequestAlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.PopularContentResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SkeletonDeltaResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SkeletonVersionResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.FullSkeletonResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.GenreResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.GenreTracksResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncStateResponse
import com.lelloman.pezzottify.android.domain.catalogsync.CatalogSyncResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.WhatsNewResponse
import okhttp3.ResponseBody
import retrofit2.Response
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.Header
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path
import retrofit2.http.Query

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
        @Path("artistId") artistId: String,
        @Query("offset") offset: Int? = null,
        @Query("limit") limit: Int? = null,
        @Query("appears_on") appearsOn: Boolean? = null,
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

    @POST("/v1/content/batch")
    suspend fun postBatchContent(
        @Header("Authorization") authToken: String,
        @Body request: BatchContentRequest,
    ): Response<BatchContentResponse>

    @GET("/v1/content/popular")
    suspend fun getPopularContent(
        @Header("Authorization") authToken: String,
        @Query("albums_limit") albumsLimit: Int,
        @Query("artists_limit") artistsLimit: Int,
    ): Response<PopularContentResponse>

    @GET("/v1/content/catalog/stats")
    suspend fun getCatalogStats(
        @Header("Authorization") authToken: String,
    ): Response<CatalogStatsResponse>

    @GET("/v1/content/whatsnew")
    suspend fun getWhatsNew(
        @Header("Authorization") authToken: String,
        @Query("limit") limit: Int,
    ): Response<WhatsNewResponse>

    @GET("/v1/content/genres")
    suspend fun getGenres(
        @Header("Authorization") authToken: String,
    ): Response<List<GenreResponse>>

    @GET("/v1/content/genre/{genreName}/tracks")
    suspend fun getGenreTracks(
        @Header("Authorization") authToken: String,
        @Path("genreName") genreName: String,
        @Query("limit") limit: Int = 20,
        @Query("offset") offset: Int = 0,
    ): Response<GenreTracksResponse>

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

    @POST("/v1/user/liked/{contentType}/{contentId}")
    suspend fun likeContent(
        @Header("Authorization") authToken: String,
        @Path("contentType") contentType: String,
        @Path("contentId") contentId: String,
    ): Response<Unit>

    @DELETE("/v1/user/liked/{contentType}/{contentId}")
    suspend fun unlikeContent(
        @Header("Authorization") authToken: String,
        @Path("contentType") contentType: String,
        @Path("contentId") contentId: String,
    ): Response<Unit>

    @POST("/v1/user/listening")
    suspend fun recordListeningEvent(
        @Header("Authorization") authToken: String,
        @Body request: ListeningEventRequest,
    ): Response<ListeningEventResponse>

    @POST("/v1/user/impression")
    suspend fun recordImpression(
        @Header("Authorization") authToken: String,
        @Body request: ImpressionRequest,
    ): Response<Unit>

    @GET("/v1/user/listening/events")
    suspend fun getListeningEvents(
        @Header("Authorization") authToken: String,
        @Query("start_date") startDate: Int? = null,
        @Query("end_date") endDate: Int? = null,
        @Query("limit") limit: Int? = null,
        @Query("offset") offset: Int? = null,
    ): Response<List<ListeningEventItem>>

    @GET("/v1/sync/state")
    suspend fun getSyncState(
        @Header("Authorization") authToken: String,
    ): Response<SyncStateResponse>

    @GET("/v1/sync/events")
    suspend fun getSyncEvents(
        @Header("Authorization") authToken: String,
        @Query("since") since: Long,
    ): Response<SyncEventsResponse>

    @GET("/v1/sync/catalog")
    suspend fun getCatalogSync(
        @Header("Authorization") authToken: String,
        @Query("since") since: Long,
    ): Response<CatalogSyncResponse>

    @PUT("/v1/user/settings")
    suspend fun updateUserSettings(
        @Header("Authorization") authToken: String,
        @Body request: UpdateUserSettingsRequest,
    ): Response<Unit>

    @GET("/v1/user/devices")
    suspend fun getDevices(
        @Header("Authorization") authToken: String,
    ): Response<DevicesResponse>

    @PUT("/v1/user/devices/{deviceId}/share_policy")
    suspend fun updateDeviceSharePolicy(
        @Header("Authorization") authToken: String,
        @Path("deviceId") deviceId: Int,
        @Body request: DeviceSharePolicyRequest,
    ): Response<DeviceSharePolicy>

    @POST("/v1/user/notifications/{notificationId}/read")
    suspend fun markNotificationRead(
        @Header("Authorization") authToken: String,
        @Path("notificationId") notificationId: String,
    ): Response<Unit>

    // Download manager endpoints

    @GET("/v1/download/limits")
    suspend fun getDownloadLimits(
        @Header("Authorization") authToken: String,
    ): Response<DownloadLimitsResponse>

    @POST("/v1/download/request/album")
    suspend fun requestAlbumDownload(
        @Header("Authorization") authToken: String,
        @Body request: RequestAlbumDownloadBody,
    ): Response<RequestAlbumResponse>

    @GET("/v1/download/my-requests")
    suspend fun getMyDownloadRequests(
        @Header("Authorization") authToken: String,
        @Query("limit") limit: Int? = null,
        @Query("offset") offset: Int? = null,
    ): Response<MyDownloadRequestsResponse>

    // Skeleton sync endpoints

    @GET("/v1/catalog/skeleton/version")
    suspend fun getSkeletonVersion(
        @Header("Authorization") authToken: String,
    ): Response<SkeletonVersionResponse>

    @GET("/v1/catalog/skeleton")
    suspend fun getFullSkeleton(
        @Header("Authorization") authToken: String,
    ): Response<FullSkeletonResponse>

    @GET("/v1/catalog/skeleton/delta")
    suspend fun getSkeletonDelta(
        @Header("Authorization") authToken: String,
        @Query("since") sinceVersion: Long,
    ): Response<SkeletonDeltaResponse>

    // Playlist endpoints

    @POST("/v1/user/playlist")
    suspend fun createPlaylist(
        @Header("Authorization") authToken: String,
        @Body request: CreatePlaylistRequest,
    ): Response<CreatePlaylistResponse>

    @PUT("/v1/user/playlist/{playlistId}")
    suspend fun updatePlaylist(
        @Header("Authorization") authToken: String,
        @Path("playlistId") playlistId: String,
        @Body request: UpdatePlaylistRequest,
    ): Response<Unit>

    @DELETE("/v1/user/playlist/{playlistId}")
    suspend fun deletePlaylist(
        @Header("Authorization") authToken: String,
        @Path("playlistId") playlistId: String,
    ): Response<Unit>

    @PUT("/v1/user/playlist/{playlistId}/add")
    suspend fun addTracksToPlaylist(
        @Header("Authorization") authToken: String,
        @Path("playlistId") playlistId: String,
        @Body request: AddTracksToPlaylistRequest,
    ): Response<Unit>

    @PUT("/v1/user/playlist/{playlistId}/remove")
    suspend fun removeTracksFromPlaylist(
        @Header("Authorization") authToken: String,
        @Path("playlistId") playlistId: String,
        @Body request: RemoveTracksFromPlaylistRequest,
    ): Response<Unit>

    // Bug report endpoint

    @POST("/v1/user/bug-report")
    suspend fun submitBugReport(
        @Header("Authorization") authToken: String,
        @Body request: SubmitBugReportRequest,
    ): Response<SubmitBugReportResponse>
}
