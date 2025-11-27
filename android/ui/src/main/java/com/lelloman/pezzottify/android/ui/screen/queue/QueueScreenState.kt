package com.lelloman.pezzottify.android.ui.screen.queue

import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.Track
import kotlinx.coroutines.flow.Flow

data class QueueScreenState(
    val isLoading: Boolean = true,
    val isError: Boolean = false,
    val contextName: String = "",
    val contextType: QueueContextType = QueueContextType.Unknown,
    val tracks: List<Flow<Content<Track>>> = emptyList(),
    val currentTrackIndex: Int? = null,
    val canSaveAsPlaylist: Boolean = false,
)

enum class QueueContextType {
    Album,
    UserPlaylist,
    UserMix,
    Unknown,
}
