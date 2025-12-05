package com.lelloman.pezzottify.android.domain.auth.usecase

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.usecase.UseCase
import com.lelloman.pezzottify.android.domain.user.UserDataStore
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import javax.inject.Inject

class PerformLogout @Inject internal constructor(
    private val authStore: AuthStore,
    private val remoteApiClient: RemoteApiClient,
    private val staticsStore: StaticsStore,
    private val staticsCache: StaticsCache,
    private val userDataStore: UserDataStore,
    private val userContentStore: UserContentStore,
    private val listeningEventStore: ListeningEventStore,
    private val player: PezzottifyPlayer,
    private val webSocketManager: WebSocketManager,
) : UseCase() {

    suspend operator fun invoke() {
        player.stop()
        webSocketManager.disconnect()
        authStore.storeAuthState(AuthState.LoggedOut)
        remoteApiClient.logout()
        staticsCache.clearAll()
        staticsStore.deleteAll()
        userDataStore.deleteAll()
        userContentStore.deleteAll()
        listeningEventStore.deleteAll()
    }
}