package com.lelloman.pezzottify.android.app

import android.app.Application
import com.lelloman.pezzottify.android.app.domain.LoginManager
import com.lelloman.pezzottify.android.app.domain.LoginStateOperationsCollector
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class PezzottifyApp : Application() {

    @Inject
    lateinit var loginStateOperationsCollector: LoginStateOperationsCollector

    @Inject
    lateinit var loginManager: LoginManager

    override fun onCreate() {
        super.onCreate()
        loginStateOperationsCollector.register(loginManager)
    }
}