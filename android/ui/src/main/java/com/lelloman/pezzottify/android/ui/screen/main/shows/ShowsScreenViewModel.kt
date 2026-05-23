package com.lelloman.pezzottify.android.ui.screen.main.shows

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class ShowsScreenViewModel @Inject constructor(
    private val remoteApiClient: RemoteApiClient,
    private val player: PezzottifyPlayer,
) : ViewModel() {

    private val mutableState = MutableStateFlow(ShowsScreenState(isLoading = true))
    val state = mutableState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch(Dispatchers.IO) {
            mutableState.value = mutableState.value.copy(isLoading = true, error = null)
            when (val response = remoteApiClient.getShows()) {
                is RemoteApiResponse.Success -> {
                    mutableState.value = ShowsScreenState(
                        shows = response.data.map {
                            ShowSummaryItem(
                                id = it.id,
                                title = it.title,
                                summary = it.summary,
                                targetDurationMinutes = it.targetDurationMinutes,
                                trackCount = it.trackCount,
                            )
                        }
                    )
                }
                is RemoteApiResponse.Error -> {
                    mutableState.value = mutableState.value.copy(
                        isLoading = false,
                        error = response.toString(),
                    )
                }
            }
        }
    }

    fun selectShow(showId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            mutableState.value = mutableState.value.copy(isLoading = true, error = null)
            when (val response = remoteApiClient.getShow(showId)) {
                is RemoteApiResponse.Success -> {
                    val show = response.data
                    mutableState.value = mutableState.value.copy(
                        isLoading = false,
                        selectedShow = ShowDetailItem(
                            id = show.id,
                            title = show.title,
                            summary = show.summary,
                            segments = show.segments.map {
                                ShowSegmentItem(
                                    id = it.id,
                                    kind = it.kind,
                                    title = it.title,
                                    trackId = it.trackId,
                                    text = it.text,
                                )
                            },
                        )
                    )
                }
                is RemoteApiResponse.Error -> {
                    mutableState.value = mutableState.value.copy(
                        isLoading = false,
                        error = response.toString(),
                    )
                }
            }
        }
    }

    fun clearSelection() {
        mutableState.value = mutableState.value.copy(selectedShow = null)
    }

    fun playShowTracks() {
        val trackIds = mutableState.value.selectedShow
            ?.segments
            ?.mapNotNull { it.trackId }
            .orEmpty()
        if (trackIds.isNotEmpty()) {
            player.loadTrackIds(trackIds)
        }
    }
}
