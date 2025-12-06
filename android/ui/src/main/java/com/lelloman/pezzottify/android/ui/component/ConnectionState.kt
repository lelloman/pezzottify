package com.lelloman.pezzottify.android.ui.component

sealed interface ConnectionState {
    data object Disconnected : ConnectionState
    data object Connecting : ConnectionState
    data class Connected(val deviceId: Int, val serverVersion: String) : ConnectionState
    data class Error(val message: String) : ConnectionState
}