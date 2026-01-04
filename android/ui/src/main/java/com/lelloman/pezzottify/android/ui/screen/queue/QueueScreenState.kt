package com.lelloman.pezzottify.android.ui.screen.queue

import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.content.TrackAvailability

data class QueueTrackItem(
    val trackId: String,
    val trackName: String,
    val albumId: String,
    val artists: List<ArtistInfo>,
    val durationSeconds: Int,
    val availability: TrackAvailability = TrackAvailability.Available,
) {
    val isPlayable: Boolean
        get() = availability.isPlayable

    val isFetching: Boolean
        get() = availability == TrackAvailability.Fetching

    val isFetchError: Boolean
        get() = availability == TrackAvailability.FetchError

    val isUnavailable: Boolean
        get() = !isPlayable
}

data class QueueScreenState(
    val isLoading: Boolean = true,
    val isError: Boolean = false,
    val contextName: String = "",
    val contextType: QueueContextType = QueueContextType.Unknown,
    val tracks: List<QueueTrackItem> = emptyList(),
    val currentTrackIndex: Int? = null,
    val canSaveAsPlaylist: Boolean = false,
)

enum class QueueContextType {
    Album,
    UserPlaylist,
    UserMix,
    Unknown,
}
