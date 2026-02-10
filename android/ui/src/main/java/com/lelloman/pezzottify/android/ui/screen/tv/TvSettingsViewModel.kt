package com.lelloman.pezzottify.android.ui.screen.tv

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class TvSettingsState(
    val userHandle: String = "Unknown",
    val deviceName: String = "Unknown device",
    val deviceType: String = "unknown",
    val connectionStatus: String = "Disconnected",
    val serverUrl: String = "",
    val serverVersion: String = "disconnected",
    val versionName: String = "",
    val gitCommit: String = "",
    val isLoggingOut: Boolean = false,
)

sealed interface TvSettingsEvent {
    data object NavigateToLogin : TvSettingsEvent
}

@HiltViewModel
class TvSettingsViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val mutableState = MutableStateFlow(TvSettingsState())
    val state: StateFlow<TvSettingsState> = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<TvSettingsEvent>()
    val events: SharedFlow<TvSettingsEvent> = mutableEvents.asSharedFlow()

    init {
        mutableState.update {
            it.copy(
                serverUrl = interactor.serverUrl(),
                versionName = interactor.versionName(),
                gitCommit = interactor.gitCommit(),
            )
        }

        viewModelScope.launch {
            combine(
                interactor.userHandle(),
                interactor.deviceName(),
                interactor.deviceType(),
                interactor.connectionStatus(),
                interactor.serverVersion(),
            ) { userHandle, deviceName, deviceType, connectionStatus, serverVersion ->
                mutableState.update {
                    it.copy(
                        userHandle = userHandle ?: "Unknown",
                        deviceName = deviceName ?: "Unknown device",
                        deviceType = deviceType ?: "unknown",
                        connectionStatus = connectionStatus,
                        serverVersion = serverVersion,
                    )
                }
            }.collect { }
        }
    }

    fun clickOnLogout() {
        if (mutableState.value.isLoggingOut) return

        mutableState.update { it.copy(isLoggingOut = true) }
        viewModelScope.launch {
            interactor.logout()
            mutableState.update { it.copy(isLoggingOut = false) }
            mutableEvents.emit(TvSettingsEvent.NavigateToLogin)
        }
    }

    interface Interactor {
        suspend fun logout()
        fun userHandle(): Flow<String?>
        fun deviceName(): Flow<String?>
        fun deviceType(): Flow<String?>
        fun connectionStatus(): Flow<String>
        fun serverUrl(): String
        fun serverVersion(): Flow<String>
        fun versionName(): String
        fun gitCommit(): String
    }
}
