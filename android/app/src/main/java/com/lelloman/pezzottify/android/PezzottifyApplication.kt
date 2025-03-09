package com.lelloman.pezzottify.android

import android.app.Application
import com.lelloman.pezzottify.android.domain.usecase.InitializeApp
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class PezzottifyApplication : Application() {

    @Inject
    lateinit var initializeApp: InitializeApp

    override fun onCreate() {
        super.onCreate()
        initializeApp()
    }
}