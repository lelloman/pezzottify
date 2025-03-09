package com.lelloman.pezzottify.android.domain.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import javax.inject.Inject

class IsLoggedInUseCase @Inject constructor(
    private val authStore: AuthStore
) : UseCase() {

    operator fun invoke(): Boolean = authStore.getAuthState().value is AuthState.LoggedIn
}