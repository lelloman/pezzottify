package com.lelloman.pezzottify.android.ui.screen.maintenance

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.remoteapi.response.ServerStatus
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class MaintenanceViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val _state = MutableStateFlow(MaintenanceScreenState())
    val state: StateFlow<MaintenanceScreenState> = _state.asStateFlow()

    private val _events = MutableSharedFlow<MaintenanceScreenEvent>(extraBufferCapacity = 1)
    val events: SharedFlow<MaintenanceScreenEvent> = _events.asSharedFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            _state.update { it.copy(isLoading = true, error = false) }
            val status = interactor.fetchStatus()
            val loggedIn = interactor.isLoggedIn()
            _state.update {
                it.copy(
                    isLoading = false,
                    isLoggedIn = loggedIn,
                    message = status?.message,
                    estimatedEnd = status?.estimatedEnd,
                    error = status == null,
                )
            }
            if (status != null && !status.enabled) {
                _events.tryEmit(MaintenanceScreenEvent.MaintenanceEnded(loggedIn))
            }
        }
    }

    fun logout() {
        viewModelScope.launch {
            _state.update { it.copy(isLoggingOut = true) }
            interactor.logout()
            _state.update { it.copy(isLoggingOut = false, isLoggedIn = false) }
            // Stay on the blocking screen until a status probe confirms maintenance
            // ended; the auth routes remain reachable, so navigating to login here
            // would let the user sign back in during maintenance.
            refresh()
        }
    }

    interface Interactor {
        suspend fun fetchStatus(): ServerStatus?
        suspend fun isLoggedIn(): Boolean
        suspend fun logout()
    }
}
