package com.lelloman.pezzottify.android.app.initializer

import com.lelloman.debuginterface.DebugServerBuilder
import com.lelloman.pezzottify.android.app.PezzottifyApp

class DebugServerInitializer(private val debugServerBuilder: DebugServerBuilder) : AppInitializer {
    override fun init(app: PezzottifyApp) {
        debugServerBuilder.start()
    }
}