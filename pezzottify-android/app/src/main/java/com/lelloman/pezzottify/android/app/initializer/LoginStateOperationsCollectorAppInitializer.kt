package com.lelloman.pezzottify.android.app.initializer

import com.lelloman.pezzottify.android.app.PezzottifyApp
import com.lelloman.pezzottify.android.app.domain.login.LoginManager
import com.lelloman.pezzottify.android.app.domain.login.LoginStateOperationsCollector

class LoginStateOperationsCollectorAppInitializer(
    private val loginStateOperationsCollector: LoginStateOperationsCollector,
    private val loginManager: LoginManager,
) : AppInitializer {
    override fun init(app: PezzottifyApp) {
        loginStateOperationsCollector.register(loginManager)
    }
}