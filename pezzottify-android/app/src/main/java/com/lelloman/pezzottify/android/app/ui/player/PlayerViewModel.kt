package com.lelloman.pezzottify.android.app.ui.player

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.app.player.PlayerManager
import com.lelloman.pezzottify.android.app.ui.Navigator
import com.lelloman.pezzottify.android.log.Logger
import com.lelloman.pezzottify.android.log.LoggerFactory
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

@HiltViewModel
class PlayerViewModel @Inject constructor(
    private val playerManager: PlayerManager,
    loggerFactory: LoggerFactory,
    private val navigator: Navigator,
) : ViewModel() {

    private val log: Logger by loggerFactory

    val state: StateFlow<PlayerState>
        get() = playerManager.state
            .map(::mapper)
            .stateIn(viewModelScope, SharingStarted.Eagerly, mapper(playerManager.state.value))

    private fun mapper(playerState: PlayerManager.State): PlayerState {
        val newState = when (playerState) {
            is PlayerManager.State.Off -> PlayerState()
            is PlayerManager.State.Playing -> PlayerState(
                isPlaying = !playerState.paused,
                trackPercent = playerState.currentPositionMs.toDouble()
                    .div(playerState.trackDurationMs.toDouble())
                    .coerceIn(0.0, 1.0)
                    .toFloat(),
            )
        }
        log.debug("Player state $playerState mapped to $newState")
        return newState
    }

    fun onTrackPercentChanged(trackPercent: Float) {
        playerManager.seek(trackPercent)
    }

    fun onPlayPauseButtonClicked() {
        playerManager.togglePlayPause()
    }

    fun onNextTrackButtonClicked() {

    }

    fun onPreviousTrackButtonClicked() {

    }

    fun onSeek(percent: Float) {

    }

    fun onBackButtonClicked() {
        navigator.navigateBack()
    }

    data class PlayerState(
        val isPlaying: Boolean = false,
        val trackPercent: Float = 0f,
        val albumName: String = "",
    )
}