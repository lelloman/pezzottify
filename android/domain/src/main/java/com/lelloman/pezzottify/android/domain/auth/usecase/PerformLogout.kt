package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import javax.inject.Inject

class PerformLogout @Inject internal constructor(
    private val authStore: AuthStore,
    private val remoteApiClient: RemoteApiClient,
    private val staticsStore: StaticsStore,
    private val userDataStore: UserDataStore,
    private val userContentStore: UserContentStore,
    private val player: PezzottifyPlayer,
) : UseCase() {

    suspend operator fun invoke() {
        player.stop()
        authStore.storeAuthState(AuthState.LoggedOut)
        remoteApiClient.logout()
        staticsStore.deleteAll()
        userDataStore.deleteAll()
        userContentStore.deleteAll()
    }
}