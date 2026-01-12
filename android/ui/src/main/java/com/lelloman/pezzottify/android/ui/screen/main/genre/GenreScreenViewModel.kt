package com.lelloman.pezzottify.android.ui.screen.main.genre

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.statics.usecase.GetGenreTracks
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

@HiltViewModel(assistedFactory = GenreScreenViewModel.Factory::class)
class GenreScreenViewModel @AssistedInject constructor(
    private val getGenreTracks: GetGenreTracks,
    private val contentResolver: ContentResolver,
    private val player: PezzottifyPlayer,
    @Assisted private val genreName: String,
) : ViewModel(), GenreScreenActions {

    private val coroutineContext = Dispatchers.IO

    private val mutableState = MutableStateFlow(GenreScreenState(genreName = genreName))
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<GenreScreenEvents>()
    val events = mutableEvents.asSharedFlow()

    // Store track IDs for shuffle play
    private var trackIds: List<String> = emptyList()

    init {
        loadGenre()
    }

    private fun loadGenre() {
        viewModelScope.launch(coroutineContext) {
            mutableState.value = mutableState.value.copy(isLoading = true, error = null)

            val result = getGenreTracks(genreName)
            result.fold(
                onSuccess = { response ->
                    trackIds = response.trackIds
                    mutableState.value = mutableState.value.copy(
                        totalTracks = response.total,
                        isLoading = false,
                    )
                    // Resolve tracks progressively
                    resolveTracksProgressively(response.trackIds)
                },
                onFailure = { error ->
                    mutableState.value = mutableState.value.copy(
                        isLoading = false,
                        error = error.message ?: "Unknown error",
                    )
                }
            )
        }
    }

    private fun resolveTracksProgressively(trackIds: List<String>) {
        trackIds.forEachIndexed { index, trackId ->
            viewModelScope.launch(coroutineContext) {
                // Resolve track and album together
                val trackFlow = contentResolver.resolveTrack(trackId)

                trackFlow.collect { trackContent ->
                    when (trackContent) {
                        is Content.Resolved -> {
                            val track = trackContent.data
                            // Get album info for image URL
                            val albumContent = contentResolver.resolveAlbum(track.albumId).first { it !is Content.Loading }
                            val albumImageUrl = when (albumContent) {
                                is Content.Resolved -> albumContent.data.imageUrl
                                else -> null
                            }
                            val albumName = when (albumContent) {
                                is Content.Resolved -> albumContent.data.name
                                else -> ""
                            }

                            val trackItem = GenreTrackItemState(
                                id = track.id,
                                name = track.name,
                                durationMs = track.durationSeconds.toLong() * 1000,
                                albumId = track.albumId,
                                albumName = albumName,
                                artistNames = track.artists.map { it.name },
                                albumImageUrl = albumImageUrl,
                            )

                            updateTrackInState(trackId, trackItem)
                        }
                        is Content.Loading -> {
                            // Add placeholder while loading
                            val placeholder = GenreTrackItemState(
                                id = trackId,
                                name = "Loading...",
                                durationMs = 0,
                                albumId = "",
                                albumName = "",
                                artistNames = emptyList(),
                                albumImageUrl = null,
                            )
                            addTrackPlaceholderIfMissing(trackId, placeholder)
                        }
                        is Content.Error -> {
                            // Skip errored tracks
                        }
                    }
                }
            }
        }
    }

    private fun addTrackPlaceholderIfMissing(trackId: String, placeholder: GenreTrackItemState) {
        val currentTracks = mutableState.value.tracks
        if (currentTracks.none { it.id == trackId }) {
            mutableState.value = mutableState.value.copy(
                tracks = currentTracks + placeholder
            )
        }
    }

    private fun updateTrackInState(trackId: String, trackItem: GenreTrackItemState) {
        val currentTracks = mutableState.value.tracks.toMutableList()
        val existingIndex = currentTracks.indexOfFirst { it.id == trackId }
        if (existingIndex >= 0) {
            currentTracks[existingIndex] = trackItem
        } else {
            currentTracks.add(trackItem)
        }
        mutableState.value = mutableState.value.copy(tracks = currentTracks)
    }

    override fun clickOnTrack(trackId: String) {
        viewModelScope.launch {
            mutableEvents.emit(GenreScreenEvents.NavigateToTrack(trackId))
        }
    }

    override fun clickOnShufflePlay() {
        if (trackIds.isNotEmpty()) {
            // Shuffle the track IDs and take up to SHUFFLE_PLAY_TRACK_COUNT tracks
            val shuffledTrackIds = trackIds.shuffled().take(SHUFFLE_PLAY_TRACK_COUNT)
            // Load the first track, then add the rest after a small delay to avoid race condition
            player.loadSingleTrack(shuffledTrackIds.first())
            if (shuffledTrackIds.size > 1) {
                viewModelScope.launch {
                    // Wait for loadSingleTrack to complete its playlist creation
                    kotlinx.coroutines.delay(100)
                    player.addTracksToPlaylist(shuffledTrackIds.drop(1))
                }
            }
        }
    }

    companion object {
        private const val SHUFFLE_PLAY_TRACK_COUNT = 20
    }

    override fun goBack() {
        viewModelScope.launch {
            mutableEvents.emit(GenreScreenEvents.NavigateBack)
        }
    }

    @AssistedFactory
    interface Factory {
        fun create(genreName: String): GenreScreenViewModel
    }
}
