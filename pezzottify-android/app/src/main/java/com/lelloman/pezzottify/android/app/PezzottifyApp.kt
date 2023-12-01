package com.lelloman.pezzottify.android.app

import android.app.Application
import com.lelloman.pezzottify.android.app.initializer.AppInitializer
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class PezzottifyApp : Application() {

    @Inject
    lateinit var initializers: Set<@JvmSuppressWildcards AppInitializer>

    override fun onCreate() {
        super.onCreate()
        initializers.forEach { it.init(this) }
    }
}