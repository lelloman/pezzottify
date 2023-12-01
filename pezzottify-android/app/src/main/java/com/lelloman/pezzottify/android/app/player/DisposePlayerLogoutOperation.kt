package com.lelloman.pezzottify.android.app.player

import com.lelloman.pezzottify.android.app.domain.login.LogoutOperation
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DisposePlayerLogoutOperation @Inject constructor(private val playerManager: PlayerManager) :
    LogoutOperation {
    override suspend fun invoke() {
        playerManager.dispose()
    }
}