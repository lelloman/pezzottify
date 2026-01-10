package com.lelloman.pezzottify.android.domain.remoteapi

import com.lelloman.pezzottify.android.domain.listening.ListeningEventSyncData
import com.lelloman.pezzottify.android.domain.remoteapi.request.BatchContentRequest
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistDiscographyResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.BatchContentResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadLimitsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.FullSkeletonResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ImageResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ListeningEventItem
import com.lelloman.pezzottify.android.domain.remoteapi.response.ListeningEventRecordedResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.LoginSuccessResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.MyDownloadRequestsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.PopularContentResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.RequestAlbumResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchSection
import com.lelloman.pezzottify.android.domain.remoteapi.response.SkeletonDeltaResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SkeletonVersionResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.SyncStateResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.WhatsNewResponse
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable


data class DeviceInfo(
    val deviceUuid: String,
    val deviceType: String,
    val deviceName: String?,
    val osInfo: String?,
)

interface RemoteApiClient {

    suspend fun login(
        userHandle: String,
        password: String,
        deviceInfo: DeviceInfo,
    ): RemoteApiResponse<LoginSuccessResponse>

    suspend fun logout(): RemoteApiResponse<Unit>

    suspend fun getArtist(artistId: String): RemoteApiResponse<ArtistResponse>

    suspend fun getArtistDiscography(
        artistId: String,
        offset: Int? = null,
        limit: Int? = null
    ): RemoteApiResponse<ArtistDiscographyResponse>

    suspend fun getAlbum(albumId: String): RemoteApiResponse<AlbumResponse>

    suspend fun getTrack(trackId: String): RemoteApiResponse<TrackResponse>

    suspend fun getImage(imageId: String): RemoteApiResponse<ImageResponse>

    /**
     * Fetch multiple content items in a single batch request.
     * Server limit: 100 items total across all types.
     *
     * @param request The batch request specifying which items to fetch
     * @return Response with maps of ID -> result for each content type
     */
    suspend fun getBatchContent(request: BatchContentRequest): RemoteApiResponse<BatchContentResponse>

    /**
     * Get popular albums and artists based on listening data from the last 7 days.
     */
    suspend fun getPopularContent(
        albumsLimit: Int = 10,
        artistsLimit: Int = 10,
    ): RemoteApiResponse<PopularContentResponse>

    /**
     * Get recent catalog updates ("What's New").
     * Returns closed batches with summaries of added/updated/deleted content.
     */
    suspend fun getWhatsNew(limit: Int = 10): RemoteApiResponse<WhatsNewResponse>

    suspend fun search(
        query: String,
        filters: List<SearchFilter>? = null
    ): RemoteApiResponse<SearchResponse>

    /**
     * Perform streaming search with SSE.
     * Returns a Flow that emits SearchSection objects as they arrive.
     * The stream completes when a Done section is received.
     *
     * @param query The search query string
     * @param excludeUnavailable If true, exclude unavailable content from results
     */
    fun streamingSearch(query: String, excludeUnavailable: Boolean = false): Flow<SearchSection>

    suspend fun getLikedContent(contentType: String): RemoteApiResponse<List<String>>

    suspend fun likeContent(contentType: String, contentId: String): RemoteApiResponse<Unit>

    suspend fun unlikeContent(contentType: String, contentId: String): RemoteApiResponse<Unit>

    suspend fun recordListeningEvent(data: ListeningEventSyncData): RemoteApiResponse<ListeningEventRecordedResponse>

    /**
     * Record an impression (page view) for content.
     * @param itemType The type of content: "artist", "album", or "track"
     * @param itemId The ID of the content item
     */
    suspend fun recordImpression(itemType: String, itemId: String): RemoteApiResponse<Unit>

    /**
     * Get user's listening history events.
     * Returns detailed listening events with pagination support.
     */
    suspend fun getListeningEvents(
        startDate: Int? = null,
        endDate: Int? = null,
        limit: Int? = null,
        offset: Int? = null,
    ): RemoteApiResponse<List<ListeningEventItem>>

    /**
     * Get full user sync state for initial sync.
     */
    suspend fun getSyncState(): RemoteApiResponse<SyncStateResponse>

    /**
     * Get sync events since a given sequence number.
     * Returns RemoteApiResponse.Error.EventsPruned if the sequence has been pruned.
     */
    suspend fun getSyncEvents(since: Long): RemoteApiResponse<SyncEventsResponse>

    /**
     * Update user settings on the server.
     * This generates a setting_changed event that will be synced to other devices.
     */
    suspend fun updateUserSettings(settings: List<UserSetting>): RemoteApiResponse<Unit>

    // Download manager endpoints

    /**
     * Get user's rate limit status for download requests.
     */
    suspend fun getDownloadLimits(): RemoteApiResponse<DownloadLimitsResponse>

    /**
     * Request download of an album from the external downloader service.
     */
    suspend fun requestAlbumDownload(
        albumId: String,
        albumName: String,
        artistName: String
    ): RemoteApiResponse<RequestAlbumResponse>

    /**
     * Get user's download requests.
     */
    suspend fun getMyDownloadRequests(
        limit: Int? = null,
        offset: Int? = null
    ): RemoteApiResponse<MyDownloadRequestsResponse>

    // Skeleton sync endpoints

    /**
     * Get current skeleton version and checksum.
     */
    suspend fun getSkeletonVersion(): RemoteApiResponse<SkeletonVersionResponse>

    /**
     * Get full catalog skeleton data.
     */
    suspend fun getFullSkeleton(): RemoteApiResponse<FullSkeletonResponse>

    /**
     * Get skeleton changes since a given version.
     * Returns RemoteApiResponse.Error.NotFound if the version is too old (pruned).
     */
    suspend fun getSkeletonDelta(sinceVersion: Long): RemoteApiResponse<SkeletonDeltaResponse>

    /**
     * Mark a notification as read.
     * Returns the updated notification.
     */
    suspend fun markNotificationRead(notificationId: String): RemoteApiResponse<Unit>

    // Playlist endpoints

    /**
     * Create a new playlist on the server.
     * Returns the server-assigned playlist ID.
     */
    suspend fun createPlaylist(name: String, trackIds: List<String>): RemoteApiResponse<String>

    /**
     * Update a playlist on the server.
     * Can update name, track list, or both.
     */
    suspend fun updatePlaylist(
        playlistId: String,
        name: String?,
        trackIds: List<String>?,
    ): RemoteApiResponse<Unit>

    /**
     * Delete a playlist from the server.
     */
    suspend fun deletePlaylist(playlistId: String): RemoteApiResponse<Unit>

    /**
     * Add tracks to an existing playlist.
     */
    suspend fun addTracksToPlaylist(playlistId: String, trackIds: List<String>): RemoteApiResponse<Unit>

    /**
     * Remove tracks from an existing playlist by their positions (0-indexed).
     */
    suspend fun removeTracksFromPlaylist(playlistId: String, positions: List<Int>): RemoteApiResponse<Unit>

    /**
     * Submit a bug report.
     * Returns the report ID on success.
     */
    suspend fun submitBugReport(
        title: String?,
        description: String,
        clientVersion: String?,
        deviceInfo: String?,
        logs: String?,
        attachments: List<String>? = null,
    ): RemoteApiResponse<SubmitBugReportResponse>

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