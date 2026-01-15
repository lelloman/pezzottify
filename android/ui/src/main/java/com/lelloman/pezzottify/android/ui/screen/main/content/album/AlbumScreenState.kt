package com.lelloman.pezzottify.android.ui.screen.main.content.album

import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import kotlinx.coroutines.flow.Flow

data class AlbumScreenState(
    val album: Album? = null,
    val tracks: List<Flow<Content<Track>>>? = null,
    val isLoading: Boolean = true,
    val isError: Boolean = false,
    val currentPlayingTrackId: String? = null,
    val isLiked: Boolean = false,
    val userPlaylists: List<UiUserPlaylist> = emptyList(),
    val downloadRequestState: DownloadRequestState = DownloadRequestState.Hidden,
)

sealed class DownloadRequestState {
    /** Album is complete or user lacks permission - nothing shown */
    data object Hidden : DownloadRequestState()
    /** Album has unavailable tracks, not yet requested - show request button */
    data object CanRequest : DownloadRequestState()
    /** Currently requesting - show loading state */
    data object Requesting : DownloadRequestState()
    /** Request submitted, waiting for server confirmation - show loading */
    data object WaitingForConfirmation : DownloadRequestState()
    /** Already in user's download queue - show "requested" label */
    data class Requested(
        val status: RequestStatus = RequestStatus.Pending,
        val queuePosition: Int? = null,
        val progress: RequestProgress? = null,
    ) : DownloadRequestState()
    /** Request failed - show error */
    data class Error(val errorType: DownloadRequestErrorType) : DownloadRequestState()
}

enum class DownloadRequestErrorType {
    Network,
    Unauthorized,
    NotFound,
    Unknown,
}

enum class RequestStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

data class RequestProgress(
    val completed: Int,
    val total: Int,
)