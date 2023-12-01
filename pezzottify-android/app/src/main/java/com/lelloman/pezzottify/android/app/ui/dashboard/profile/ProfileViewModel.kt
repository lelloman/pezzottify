package com.lelloman.pezzottify.android.app.ui.dashboard.profile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.app.domain.login.LoginManager
import com.lelloman.pezzottify.android.app.ui.Navigator
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import javax.inject.Inject

@HiltViewModel
class ProfileViewModel @Inject constructor(
    private val loginManager: LoginManager,
    private val navigator: Navigator,
) : ViewModel() {

    private val mutableState = MutableStateFlow(State())
    val state: StateFlow<State> = mutableState.asStateFlow()

    fun onLogoutButtonClicked() {
        runBlocking {
            mutableState.value = state.value.copy(showLogoutConfirmation = true)
        }
    }

    fun onDismissDialog() {
        runBlocking {
            mutableState.value = state.value.copy(showLogoutConfirmation = false)
        }
    }

    fun onLogoutConfirmed() = viewModelScope.launch {
        mutableState.value = state.value.copy(showLogoutConfirmation = true)
        loginManager.logout()
        navigator.fromProfileToLogin()
    }

    data class State(
        val showLogoutConfirmation: Boolean = false,
    )
}