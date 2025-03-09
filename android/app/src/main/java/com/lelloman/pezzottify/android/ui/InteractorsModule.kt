package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.usecase.IsLoggedInUseCase
import com.lelloman.pezzottify.android.domain.usecase.PerformLoginUseCase
import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.splash.SplashViewModel
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.components.ViewModelComponent
import kotlinx.coroutines.delay
import kotlin.time.Duration.Companion.seconds

@InstallIn(ViewModelComponent::class)
@Module
class InteractorsModule {

    @Provides
    fun provideSplashInteractor(isLoggedIn: IsLoggedInUseCase): SplashViewModel.Interactor =
        object : SplashViewModel.Interactor {
            override suspend fun isLoggedIn() = isLoggedIn()
        }

    @Provides
    fun provideLoginInteractor(
        performLogin: PerformLoginUseCase,
    ): LoginViewModel.Interactor = object : LoginViewModel.Interactor {
        override suspend fun login(
            email: String,
            password: String
        ): LoginViewModel.Interactor.LoginResult =
            when (performLogin(email, password)) {
                PerformLoginUseCase.LoginResult.Success -> LoginViewModel.Interactor.LoginResult.Success
                PerformLoginUseCase.LoginResult.WrongCredentials -> LoginViewModel.Interactor.LoginResult.Failure.InvalidCredentials
                PerformLoginUseCase.LoginResult.Error -> LoginViewModel.Interactor.LoginResult.Failure.Unknown
            }
    }

    @Provides
    fun provideProfileScreenInteractor() = object : ProfileScreenViewModel.Interactor {
        override suspend fun logout() {
            delay(2.seconds)
        }
    }
}