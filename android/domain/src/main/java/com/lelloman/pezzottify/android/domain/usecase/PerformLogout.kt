package com.lelloman.pezzottify.android.domain.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import javax.inject.Inject

class PerformLogout @Inject constructor(
    private val authStore: AuthStore,
    private val remoteApiClient: RemoteApiClient,
    private val staticsStore: StaticsStore,
) : UseCase() {

    suspend operator fun invoke() {
        authStore.storeAuthState(AuthState.LoggedOut)
        remoteApiClient.logout()
        staticsStore.deleteAll()
    }
}