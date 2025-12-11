package com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toArtist
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlin.coroutines.CoroutineContext

@HiltViewModel(assistedFactory = ExternalAlbumScreenViewModel.Factory::class)
class ExternalAlbumScreenViewModel @AssistedInject constructor(
    private val interactor: Interactor,
    @Assisted private val albumId: String,
    @Assisted private val navController: NavController,
) : ViewModel(), ExternalAlbumScreenActions {

    private val _state = MutableStateFlow(ExternalAlbumScreenState())
    val state: StateFlow<ExternalAlbumScreenState> = _state.asStateFlow()

    init {
        loadAlbumDetails()
        observeDownloadStatus()
    }

    private fun loadAlbumDetails() {
        viewModelScope.launch(Dispatchers.IO) {
            _state.update { it.copy(isLoading = true, errorRes = null, errorMessage = null) }

            interactor.getExternalAlbumDetails(albumId)
                .onSuccess { album ->
                    _state.update {
                        it.copy(
                            isLoading = false,
                            album = album,
                            requestStatus = album.requestStatus,
                            errorRes = null,
                            errorMessage = null,
                        )
                    }
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(
                            isLoading = false,
                            errorMessage = error.message,
                        )
                    }
                }
        }
    }

    private fun observeDownloadStatus() {
        viewModelScope.launch {
            interactor.observeDownloadStatus(albumId).collect { status ->
                if (status != null) {
                    _state.update { it.copy(requestStatus = status) }
                }
            }
        }
    }

    override fun requestDownload() {
        val album = _state.value.album ?: return

        viewModelScope.launch(Dispatchers.IO) {
            _state.update { it.copy(isRequesting = true, errorRes = null, errorMessage = null) }

            interactor.requestAlbumDownload(
                albumId = album.id,
                albumName = album.name,
                artistName = album.artistName,
            )
                .onSuccess { status ->
                    _state.update {
                        it.copy(
                            isRequesting = false,
                            requestStatus = status,
                        )
                    }
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(
                            isRequesting = false,
                            errorRes = R.string.failed_to_request_download,
                        )
                    }
                }
        }
    }

    override fun navigateToArtist() {
        val artistId = _state.value.album?.artistId ?: return
        navController.toArtist(artistId)
    }

    override fun navigateToCatalogAlbum() {
        // Album is in catalog, navigate to regular album screen
        navController.toAlbum(albumId)
    }

    override fun retry() {
        loadAlbumDetails()
    }

    @AssistedFactory
    interface Factory {
        fun create(albumId: String, navController: NavController): ExternalAlbumScreenViewModel
    }

    /**
     * Interactor interface for the ExternalAlbumScreen.
     * Implementation is provided via Hilt from InteractorsModule.
     */
    interface Interactor {
        suspend fun getExternalAlbumDetails(albumId: String): Result<UiExternalAlbumWithStatus>
        suspend fun requestAlbumDownload(
            albumId: String,
            albumName: String,
            artistName: String,
        ): Result<UiRequestStatus>
        fun observeDownloadStatus(albumId: String): Flow<UiRequestStatus?>
    }
}
