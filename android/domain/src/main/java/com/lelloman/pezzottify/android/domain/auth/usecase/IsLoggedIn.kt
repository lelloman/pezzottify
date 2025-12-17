package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import kotlinx.coroutines.flow.first
import javax.inject.Inject

class IsLoggedIn @Inject constructor(
    private val authStore: AuthStore
) : UseCase() {

    /**
     * Returns true if the user is logged in.
     * Suspends until auth state is resolved (not Loading).
     */
    suspend operator fun invoke(): Boolean {
        val authState = authStore.getAuthState().first { it !is AuthState.Loading }
        return authState is AuthState.LoggedIn
    }
}