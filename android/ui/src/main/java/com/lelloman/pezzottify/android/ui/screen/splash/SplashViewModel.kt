package com.lelloman.pezzottify.android.ui.screen.splash

import androidx.lifecycle.ViewModel
import com.lelloman.pezzottify.android.domain.remoteapi.response.ServerStatus
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.flow
import javax.inject.Inject

@HiltViewModel
class SplashViewModel @Inject constructor(private val interactor: Interactor) : ViewModel() {

    val destination
        get() = flow {
            if (interactor.serverStatus()?.enabled == true) {
                emit(Destination.Maintenance)
            } else if (interactor.isLoggedIn()) {
                emit(Destination.Main)
            } else {
                emit(Destination.Login)
            }
        }

    enum class Destination {
        Login,
        Main,
        Maintenance,
    }

    interface Interactor {
        suspend fun isLoggedIn(): Boolean
        suspend fun serverStatus(): ServerStatus?
    }
}