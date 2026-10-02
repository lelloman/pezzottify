package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import javax.inject.Inject

/** Gravity of the queue being controlled, whether it plays here or on a remote device. */
data class PlaybackGravityState(
    val gravity: Gravity? = null,
    val trackIds: List<String> = emptyList(),
    val hasPlaylist: Boolean = false,
    val isRadio: Boolean = false,
    val isRemote: Boolean = false,
) {
    /** Gravity applies only to non-radio queues. */
    val isSteerable: Boolean get() = hasPlaylist && !isRadio
}

class PlaybackGravity @Inject constructor(
    private val player: PezzottifyPlayer,
    private val playbackModeManager: PlaybackModeManager,
    private val playbackSessionHandler: PlaybackSessionHandler,
) {

    val state: Flow<PlaybackGravityState> = combine(
        playbackModeManager.mode,
        player.playbackPlaylist,
        playbackSessionHandler.otherDeviceGravities,
        playbackSessionHandler.otherDeviceQueues,
        playbackSessionHandler.otherDeviceQueueContexts,
    ) { _, _, _, _, _ -> current() }.distinctUntilChanged()

    fun current(): PlaybackGravityState = when (val mode = playbackModeManager.mode.value) {
        is PlaybackMode.Local -> {
            val playlist = player.playbackPlaylist.value
            val isRadio = playlist?.context is PlaybackPlaylistContext.Radio
            PlaybackGravityState(
                gravity = if (playlist == null || isRadio) null else playlist.gravity ?: Gravity(),
                trackIds = playlist?.tracksIds.orEmpty(),
                hasPlaylist = playlist != null,
                isRadio = isRadio,
            )
        }

        is PlaybackMode.Remote -> {
            val trackIds = playbackSessionHandler.otherDeviceQueues.value[mode.deviceId].orEmpty()
            val isRadio = playbackSessionHandler.otherDeviceQueueContexts.value[mode.deviceId] is
                PlaybackPlaylistContext.Radio
            val hasPlaylist = trackIds.isNotEmpty()
            PlaybackGravityState(
                gravity = if (!hasPlaylist || isRadio) null
                else playbackSessionHandler.otherDeviceGravities.value[mode.deviceId] ?: Gravity(),
                trackIds = trackIds,
                hasPlaylist = hasPlaylist,
                isRadio = isRadio,
                isRemote = true,
            )
        }
    }

    /** Applies [transform] to the current gravity. Returns false when the queue cannot be steered. */
    fun update(transform: (Gravity) -> Gravity): Boolean {
        val gravity = current().gravity ?: return false
        player.setGravity(transform(gravity))
        return true
    }
}

/**
 * Sets [reference] as destination. Explore mode deliberately avoids the closest matches and would
 * never arrive, so it falls back to the default mode when a destination is set.
 */
fun Gravity.steeringToward(reference: GravityReference, stepsTotal: Int): Gravity {
    val next = withDestination(reference, stepsTotal)
    return if (next.knobs.mode == MODE_EXPLORE) next.withKnobs(next.knobs.copy(mode = null)) else next
}

const val MODE_EXPLORE = "explore"
