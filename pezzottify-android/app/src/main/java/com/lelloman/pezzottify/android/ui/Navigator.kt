package com.lelloman.pezzottify.android.ui

import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class Navigator @Inject constructor() {

    private val _channel: Channel<NavigationEvent> = Channel(
        capacity = Int.MAX_VALUE,
        onBufferOverflow = BufferOverflow.DROP_LATEST,
    )
    val channel: ReceiveChannel<NavigationEvent> = _channel

    fun fromSplashToHome() {
        val event =
            NavigationEvent.GoTo(Routes.Dashboard.route, NavigationEvent.PopUpTo(Routes.Splash.route))
        _channel.trySend(event)
    }

    fun fromSplashToLogin() {
        val event =
            NavigationEvent.GoTo(Routes.Login.route, NavigationEvent.PopUpTo(Routes.Splash.route))
        _channel.trySend(event)
    }

    fun fromLoginToHome() {
        val event =
            NavigationEvent.GoTo(Routes.Dashboard.route, NavigationEvent.PopUpTo(Routes.Login.route))
        _channel.trySend(event)
    }
}

sealed class NavigationEvent {
    object GoBack : NavigationEvent()

    data class GoTo(val route: String, val popUpTo: PopUpTo? = null) : NavigationEvent()

    data class PopUpTo(val route: String, val inclusive: Boolean = true)
}


sealed class Routes(val route: String) {
    object Splash : Routes("splash")
    object Dashboard : Routes("dashboard") {
        object Home : Routes("home/home")
        object Search : Routes("home/search")
        object Profile : Routes("home/profile")
    }

    object Login : Routes("login")
}