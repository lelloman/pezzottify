package com.lelloman.pezzottify.android.app.debugcommands

import com.lelloman.debuginterface.DebugOperation
import com.lelloman.pezzottify.android.app.domain.login.LoginManager
import com.lelloman.pezzottify.android.app.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.app.ui.Navigator
import kotlinx.coroutines.runBlocking

private fun logoutCommand(loginManager: LoginManager, navigator: Navigator) =
    DebugOperation.SimpleAction("Logout") {
        runBlocking {
            loginManager.logout()
            navigator.restartFromSplash()
        }
    }

private fun refreshStaticsCommand(staticsStore: StaticsStore) =
    DebugOperation.SimpleAction(
        name = "Refresh statics",
        description = "Download artists and albums to local storage"
    ) {
        runBlocking { staticsStore.fetchStatics() }
    }

fun makeDebugOperationsList(
    loginManager: LoginManager,
    staticsStore: StaticsStore,
    navigator: Navigator,
) = mutableListOf<DebugOperation>().apply {
    add(logoutCommand(loginManager, navigator))
    add(refreshStaticsCommand(staticsStore))
}