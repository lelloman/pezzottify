package com.lelloman.pezzottify.android.lifecycle

import android.app.Service
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class KeepAliveService : Service() {

    @Inject
    lateinit var appLifecycleObserver: AndroidAppLifecycleObserver

    override fun onBind(intent: Intent?): IBinder {
        appLifecycleObserver.setKeptAliveExternally(true)
        return Binder()
    }

    override fun onUnbind(intent: Intent?): Boolean {
        appLifecycleObserver.setKeptAliveExternally(false)
        return false
    }
}
