package com.lelloman.pezzottify.android.app.ui.dashboard

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.app.player.PlayerManager
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

@HiltViewModel
class DashboardViewModel @Inject constructor(
    private val playerManager: PlayerManager,
) : ViewModel() {

    val state: StateFlow<State>
        get() = playerManager.state
            .map(::mapper)
            .stateIn(viewModelScope, SharingStarted.Eagerly, mapper(playerManager.state.value))

    private fun mapper(playerState: PlayerManager.State): State {
        val controlsState = when (playerState) {
            is PlayerManager.State.Off -> null
            is PlayerManager.State.Playing -> PlayerControlsState(
                isPlaying = !playerState.paused,
                trackPercent = playerState.currentTimeMs.toDouble()
                    .div(playerState.trackDurationMs.toDouble())
                    .coerceIn(0.0, 1.0)
                    .toFloat(),
            )
        }
        return State(controlsState)
    }

    fun onPlayPauseButtonClicked() {
        playerManager.togglePlayPause()
    }

    data class State(
        val playerControlsState: PlayerControlsState?
    )

    data class PlayerControlsState(
        val isPlaying: Boolean = false,
        val trackPercent: Float = 0f,
    )
}