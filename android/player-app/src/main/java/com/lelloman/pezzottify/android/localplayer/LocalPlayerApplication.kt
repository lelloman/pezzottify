package com.lelloman.pezzottify.android.localplayer

import android.app.Application
import com.lelloman.pezzottify.android.player.equalizer.AndroidEqualizerOutputController
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class LocalPlayerApplication : Application() {
    @Inject lateinit var equalizerOutputs: AndroidEqualizerOutputController

    override fun onCreate() {
        super.onCreate()
        equalizerOutputs.start()
    }
}
