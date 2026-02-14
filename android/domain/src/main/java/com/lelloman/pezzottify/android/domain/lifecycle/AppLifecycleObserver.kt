package com.lelloman.pezzottify.android.domain.lifecycle

import kotlinx.coroutines.flow.StateFlow

interface AppLifecycleObserver {
    val isInForeground: StateFlow<Boolean>
    val isKeptAliveExternally: StateFlow<Boolean>
}
