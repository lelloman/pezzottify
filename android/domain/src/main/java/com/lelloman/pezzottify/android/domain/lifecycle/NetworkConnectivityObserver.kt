package com.lelloman.pezzottify.android.domain.lifecycle

import kotlinx.coroutines.flow.StateFlow

interface NetworkConnectivityObserver {
    val isNetworkAvailable: StateFlow<Boolean>
}
