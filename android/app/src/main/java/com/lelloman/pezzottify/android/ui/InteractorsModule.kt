package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.screen.splash.SplashViewModel
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.components.ViewModelComponent
import kotlinx.coroutines.delay
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

@InstallIn(ViewModelComponent::class)
@Module
class InteractorsModule {

    @Provides
    fun provideSplashInteractor(): SplashViewModel.Interactor =
        object : SplashViewModel.Interactor {
            override suspend fun isLoggedIn(): Boolean {
                delay(1.seconds)
                return false
            }
        }

    @Provides
    fun provideLoginInteractor(): LoginViewModel.Interactor = object : LoginViewModel.Interactor {
        override suspend fun login(
            host: String,
            email: String,
            password: String
        ): LoginViewModel.Interactor.LoginResult {
            delay(500.milliseconds)
            return LoginViewModel.Interactor.LoginResult.Success
        }
    }
}