package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.usecase.IsLoggedIn
import com.lelloman.pezzottify.android.domain.usecase.PerformLogin
import com.lelloman.pezzottify.android.domain.usecase.PerformLogout
import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.splash.SplashViewModel
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.components.ViewModelComponent

@InstallIn(ViewModelComponent::class)
@Module
class InteractorsModule {

    @Provides
    fun provideSplashInteractor(isLoggedIn: IsLoggedIn): SplashViewModel.Interactor =
        object : SplashViewModel.Interactor {
            override suspend fun isLoggedIn() = isLoggedIn()
        }

    @Provides
    fun provideLoginInteractor(
        performLogin: PerformLogin,
    ): LoginViewModel.Interactor = object : LoginViewModel.Interactor {
        override suspend fun login(
            email: String,
            password: String
        ): LoginViewModel.Interactor.LoginResult =
            when (performLogin(email, password)) {
                PerformLogin.LoginResult.Success -> LoginViewModel.Interactor.LoginResult.Success
                PerformLogin.LoginResult.WrongCredentials -> LoginViewModel.Interactor.LoginResult.Failure.InvalidCredentials
                PerformLogin.LoginResult.Error -> LoginViewModel.Interactor.LoginResult.Failure.Unknown
            }
    }

    @Provides
    fun provideProfileScreenInteractor(
        performLogout: PerformLogout,
    ): ProfileScreenViewModel.Interactor = object : ProfileScreenViewModel.Interactor {
        override suspend fun logout() {
            performLogout()
        }
    }
}