package com.lelloman.pezzottify.android.ui.screen.tv

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import javax.inject.Inject

data class TvNowPlayingStatusState(
    val userHandle: String = "Unknown",
    val connectionStatus: String = "Disconnected",
    val deviceName: String = "Unknown device",
    val deviceType: String = "unknown",
)

@HiltViewModel
class TvNowPlayingStatusViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val mutableState = MutableStateFlow(TvNowPlayingStatusState())
    val state: StateFlow<TvNowPlayingStatusState> = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            combine(
                interactor.userHandle(),
                interactor.connectionStatus(),
                interactor.deviceName(),
                interactor.deviceType(),
            ) { userHandle, connectionStatus, deviceName, deviceType ->
                TvNowPlayingStatusState(
                    userHandle = userHandle ?: "Unknown",
                    connectionStatus = connectionStatus,
                    deviceName = deviceName ?: "Unknown device",
                    deviceType = deviceType ?: "unknown",
                )
            }.collect { newState ->
                mutableState.value = newState
            }
        }
    }

    interface Interactor {
        fun userHandle(): kotlinx.coroutines.flow.Flow<String?>
        fun connectionStatus(): kotlinx.coroutines.flow.Flow<String>
        fun deviceName(): kotlinx.coroutines.flow.Flow<String?>
        fun deviceType(): kotlinx.coroutines.flow.Flow<String?>
    }
}
