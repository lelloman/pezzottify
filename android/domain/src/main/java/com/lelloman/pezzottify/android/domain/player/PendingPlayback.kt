package com.lelloman.pezzottify.android.domain.player

/**
 * Playback that has been requested but whose queue does not exist yet (e.g. a radio being
 * created server side). It is part of the player state so the UI can render it as the current
 * item from the moment creation starts until it commits into a real playlist or fails.
 */
data class PendingPlayback(
    val status: Status,
    val context: PlaybackPlaylistContext.Radio,
) {
    sealed interface Status {
        data object Creating : Status
        data class Failed(val reason: FailureReason) : Status
    }

    enum class FailureReason { Error, TimedOut, Empty }

    val isCreating: Boolean get() = status == Status.Creating
}
