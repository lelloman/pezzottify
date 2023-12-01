package com.lelloman.pezzottify.android.app.initializer

import com.lelloman.pezzottify.android.app.PezzottifyApp

fun interface AppInitializer {
    fun init(app: PezzottifyApp)
}