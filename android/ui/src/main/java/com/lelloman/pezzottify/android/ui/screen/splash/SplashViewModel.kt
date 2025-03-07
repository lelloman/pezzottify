package com.lelloman.pezzottify.android.ui.screen.splash

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.flow
import javax.inject.Inject

@HiltViewModel
class SplashViewModel @Inject constructor(private val interactor: Interactor) : ViewModel() {

    val destination
        get() = flow {
            if (interactor.isLoggedIn()) {
                emit(Destination.Main)
            } else {
                emit(Destination.Login)
            }
        }

    enum class Destination {
        Login,
        Main,
    }

    interface Interactor {
        suspend fun isLoggedIn(): Boolean
    }
}