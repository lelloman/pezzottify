package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.localdata.auth.AuthState
import com.lelloman.pezzottify.android.localdata.auth.AuthStore
import com.lelloman.pezzottify.android.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.remoteapi.response.RemoteApiResponse
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
    fun provideSplashInteractor(): SplashViewModel.Interactor =
        object : SplashViewModel.Interactor {
            override suspend fun isLoggedIn(): Boolean = true/*{
                delay(1.seconds)
                return false
            }*/
        }

    @Provides
    fun provideLoginInteractor(
        remoteApiClient: RemoteApiClient,
        authStore: AuthStore,
    ): LoginViewModel.Interactor = object : LoginViewModel.Interactor {
        override suspend fun login(
            host: String,
            email: String,
            password: String
        ): LoginViewModel.Interactor.LoginResult {
            return when (val loginResult = remoteApiClient
                .login(email, password)) {
                is RemoteApiResponse.Success -> {
                    authStore.storeAuthState(
                        AuthState.LoggedIn(
                            email,
                            loginResult.data.token,
                            host
                        )
                    )
                    return LoginViewModel.Interactor.LoginResult.Success
                }
                RemoteApiResponse.Error.Unauthorized -> LoginViewModel.Interactor.LoginResult.Failure.InvalidCredentials
                else -> LoginViewModel.Interactor.LoginResult.Failure.Unknown
            }
        }
    }

    @Provides
    fun provideProfileScreenInteractor() = object : ProfileScreenViewModel.Interactor {
        override suspend fun logout() {
            delay(2.seconds)
        }
    }
}