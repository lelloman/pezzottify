package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import javax.inject.Inject

class IsLoggedIn @Inject constructor(
    private val authStore: AuthStore
) : UseCase() {

    operator fun invoke(): Boolean = authStore.getAuthState().value is AuthState.LoggedIn
}