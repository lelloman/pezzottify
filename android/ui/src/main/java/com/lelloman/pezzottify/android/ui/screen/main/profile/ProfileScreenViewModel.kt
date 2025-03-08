package com.lelloman.pezzottify.android.ui.screen.main.profile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class ProfileScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), ProfileScreenActions {

    private val mutableEvents = MutableSharedFlow<ProfileScreenEvents>()
    val events: SharedFlow<ProfileScreenEvents> = mutableEvents.asSharedFlow()

    private var isLoading = false

    override fun clickOnLogout() {
        if (!isLoading) {
            isLoading = true
            viewModelScope.launch {
                interactor.logout()
                isLoading = false
                mutableEvents.emit(ProfileScreenEvents.NavigateToLoginScreen)
            }
        }
    }

    interface Interactor {
        suspend fun logout()
    }
}