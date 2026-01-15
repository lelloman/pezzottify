package com.lelloman.pezzottify.android.ui.screen.main.content.album

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import com.lelloman.pezzottify.android.ui.toFullScreenImage
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import com.lelloman.pezzottify.android.domain.download.RequestAlbumDownloadErrorType
import com.lelloman.pezzottify.android.domain.download.RequestAlbumDownloadException
import com.lelloman.pezzottify.android.ui.content.AlbumAvailability
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

@HiltViewModel(assistedFactory = AlbumScreenViewModel.Factory::class)
class AlbumScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    @Assisted private val albumId: String,
    @Assisted private val navController: NavController,
) : ViewModel(), AlbumScreenActions {

    private var hasLoggedView = false

    // Local state for "Requesting" status before server confirms
    private val localDownloadState = MutableStateFlow<DownloadRequestState?>(null)

    // Observe download request status only when album has unavailable tracks
    @OptIn(ExperimentalCoroutinesApi::class)
    private val downloadRequestStatusFlow = contentResolver.resolveAlbum(albumId)
        .flatMapLatest { albumContent ->
            when (albumContent) {
                is Content.Resolved -> {
                    if (albumContent.data.availability != AlbumAvailability.Complete) {
                        // Album has unavailable tracks, observe download status
                        combine(
                            interactor.hasRequestContentPermission(),
                            interactor.observeDownloadRequestStatus(albumId),
                            localDownloadState,
                        ) { hasPermission, serverStatus, localState ->
                            // Local state takes precedence during requesting and confirmation waiting
                            when {
                                localState != null && localState != DownloadRequestState.WaitingForConfirmation -> localState
                                !hasPermission -> DownloadRequestState.Hidden
                                serverStatus != null -> {
                                    // Server status received - clear local state
                                    localDownloadState.update { null }
                                    DownloadRequestState.Requested(
                                        status = serverStatus.status,
                                        queuePosition = serverStatus.queuePosition,
                                        progress = serverStatus.progress,
                                    )
                                }
                                localState == DownloadRequestState.WaitingForConfirmation -> DownloadRequestState.Requesting
                                else -> DownloadRequestState.CanRequest
                            }
                        }
                    } else {
                        flowOf(DownloadRequestState.Hidden)
                    }
                }
                else -> flowOf(DownloadRequestState.Hidden)
            }
        }

    val state = contentResolver.resolveAlbum(albumId)
        .map {
            when (it) {
                is Content.Loading -> AlbumScreenState()
                is Content.Error -> AlbumScreenState(
                    isError = true,
                    isLoading = false,
                )

                is Content.Resolved -> {
                    val trackIds = it.data.discs.flatMap { disc -> disc.tracksIds }
                    val tracks = trackIds.map { trackId -> contentResolver.resolveTrack(trackId) }
                    AlbumScreenState(
                        album = it.data,
                        tracks = tracks,
                        isError = false,
                        isLoading = false,
                    )
                }
            }
        }
        .combine(interactor.getCurrentPlayingTrackId()) { albumState, currentTrackId ->
            albumState.copy(currentPlayingTrackId = currentTrackId)
        }
        .combine(interactor.isLiked(albumId)) { albumState, isLiked ->
            albumState.copy(isLiked = isLiked)
        }
        .combine(interactor.getUserPlaylists()) { albumState, playlists ->
            albumState.copy(userPlaylists = playlists)
        }
        .combine(downloadRequestStatusFlow) { albumState, downloadState ->
            albumState.copy(downloadRequestState = downloadState)
        }
        .onEach { state ->
            if (!state.isLoading && !state.isError && !hasLoggedView) {
                hasLoggedView = true
                interactor.logViewedAlbum(albumId)
            }
        }
        .stateIn(viewModelScope, SharingStarted.Eagerly, AlbumScreenState())

    override fun clickOnPlayAlbum(albumId: String) {
        interactor.playAlbum(albumId)
    }

    override fun clickOnTrack(trackId: String) {
        // When clicking a track directly, play it (replaces current queue with album starting from track)
        interactor.playTrack(albumId, trackId)
    }

    override fun clickOnAlbumImage(imageUrl: String?) {
        if (imageUrl != null) {
            navController.toFullScreenImage(imageUrl)
        }
    }

    override fun clickOnLike() {
        interactor.toggleLike(albumId, state.value.isLiked)
    }

    override fun playTrackDirectly(trackId: String) {
        // Play only this single track (not the album context)
        interactor.playTrackDirectly(trackId)
    }

    override fun addTrackToQueue(trackId: String) {
        interactor.addTrackToQueue(trackId)
    }

    override fun addAlbumToQueue(albumId: String) {
        interactor.addAlbumToQueue(albumId)
    }

    override fun addTrackToPlaylist(trackId: String, playlistId: String) {
        viewModelScope.launch {
            interactor.addTrackToPlaylist(trackId, playlistId)
        }
    }

    override fun addAlbumToPlaylist(albumId: String, playlistId: String) {
        viewModelScope.launch {
            interactor.addAlbumToPlaylist(albumId, playlistId)
        }
    }

    override fun createPlaylist(name: String) {
        viewModelScope.launch {
            interactor.createPlaylist(name)
        }
    }

    override fun toggleTrackLike(trackId: String, currentlyLiked: Boolean) {
        interactor.toggleTrackLike(trackId, currentlyLiked)
    }

    override fun getTrackLikeState(trackId: String): Flow<Boolean> = interactor.isLiked(trackId)

    override fun requestDownload() {
        viewModelScope.launch {
            val currentState = state.value
            val album = currentState.album ?: return@launch

            // Set local state to Requesting
            localDownloadState.update { DownloadRequestState.Requesting }

            try {
                // Resolve first artist name for the download request
                val firstArtistId = album.artistsIds.firstOrNull()
                val artistName = if (firstArtistId != null) {
                    val artistContent = contentResolver.resolveArtist(firstArtistId).first()
                    if (artistContent is Content.Resolved) artistContent.data.name else "Unknown"
                } else {
                    "Unknown"
                }

                val result = interactor.requestAlbumDownload(albumId, album.name, artistName)
                if (result.isSuccess) {
                    // Set state to WaitingForConfirmation to prevent flicker
                    // Server status will update shortly via WebSocket
                    localDownloadState.update { DownloadRequestState.WaitingForConfirmation }
                } else {
                    val errorType = (result.exceptionOrNull() as? RequestAlbumDownloadException)
                        ?.errorType
                        ?.toUiErrorType()
                        ?: DownloadRequestErrorType.Unknown
                    localDownloadState.update { DownloadRequestState.Error(errorType) }
                }
            } catch (e: RequestAlbumDownloadException) {
                localDownloadState.update { DownloadRequestState.Error(e.errorType.toUiErrorType()) }
            } catch (e: Exception) {
                localDownloadState.update { DownloadRequestState.Error(DownloadRequestErrorType.Unknown) }
            }
        }
    }

    interface Interactor {
        fun playAlbum(albumId: String)
        fun playTrack(albumId: String, trackId: String)
        fun logViewedAlbum(albumId: String)
        fun getCurrentPlayingTrackId(): Flow<String?>
        fun isLiked(contentId: String): Flow<Boolean>
        fun toggleLike(contentId: String, currentlyLiked: Boolean)
        fun toggleTrackLike(trackId: String, currentlyLiked: Boolean)
        fun getUserPlaylists(): Flow<List<UiUserPlaylist>>

        // Methods for bottom sheet actions
        fun playTrackDirectly(trackId: String)
        fun addTrackToQueue(trackId: String)
        fun addAlbumToQueue(albumId: String)
        suspend fun addTrackToPlaylist(trackId: String, playlistId: String)
        suspend fun addAlbumToPlaylist(albumId: String, playlistId: String)
        suspend fun createPlaylist(name: String)

        // Methods for download request
        fun hasRequestContentPermission(): Flow<Boolean>
        fun observeDownloadRequestStatus(albumId: String): Flow<DownloadRequestStatus?>
        suspend fun requestAlbumDownload(albumId: String, albumName: String, artistName: String): Result<Unit>
    }

    /** Status of a download request from the server */
    data class DownloadRequestStatus(
        val status: RequestStatus,
        val queuePosition: Int? = null,
        val progress: RequestProgress? = null,
    )

    @AssistedFactory
    interface Factory {
        fun create(albumId: String, navController: NavController): AlbumScreenViewModel
    }
}

private fun RequestAlbumDownloadErrorType.toUiErrorType(): DownloadRequestErrorType = when (this) {
    RequestAlbumDownloadErrorType.Network -> DownloadRequestErrorType.Network
    RequestAlbumDownloadErrorType.Unauthorized -> DownloadRequestErrorType.Unauthorized
    RequestAlbumDownloadErrorType.NotFound -> DownloadRequestErrorType.NotFound
    RequestAlbumDownloadErrorType.Unknown -> DownloadRequestErrorType.Unknown
}
