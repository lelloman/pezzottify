package com.lelloman.pezzottify.android.app.ui.dashboard

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.app.player.PlayerManager
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import javax.inject.Inject

@HiltViewModel
class DashboardViewModel @Inject constructor(
    private val playerManager: PlayerManager,
) : ViewModel() {

    private val mutableState = MutableStateFlow(State(playerControlsState = null))
    val state = mutableState.asStateFlow()

    init {
        viewModelScope.run {
            playerManager.state
                .map { playerState ->
                    val controlsState = when (playerState) {
                        is PlayerManager.State.Off -> null
                        is PlayerManager.State.Playing -> PlayerControlsState(
                            isPlaying = !playerState.paused,
                        )
                    }
                    mutableState.emit(state.value.copy(playerControlsState = controlsState))
                }
        }
    }

    data class State(
        val playerControlsState: PlayerControlsState?
    )

    data class PlayerControlsState(
        val isPlaying: Boolean = false,
    )
}