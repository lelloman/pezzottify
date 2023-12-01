package com.lelloman.pezzottify.android.app.ui

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

    fun restartFromSplash() {
        val event = NavigationEvent.GoTo(
            Routes.Splash.route,
            NavigationEvent.PopUp.All
        )
        _channel.trySend(event)
    }

    fun fromSplashToHome() {
        val event = NavigationEvent.GoTo(
            Routes.Dashboard.route, NavigationEvent.PopUp.To(Routes.Splash.route)
        )
        _channel.trySend(event)
    }

    fun fromSplashToLogin() {
        val event = NavigationEvent.GoTo(
            Routes.Login.route, NavigationEvent.PopUp.To(Routes.Splash.route)
        )
        _channel.trySend(event)
    }

    fun fromLoginToHome() {
        Routes.Dashboard.destination().popUpTo(Routes.Login).go()
    }

    fun fromProfileToLogin() {
        Routes.Login.destination().popUpTo(Routes.Dashboard).go()
    }

    fun fromDashboardToPlayer() {
        Routes.Player.destination().go()
    }

    fun navigateBack() {
        _channel.trySend(NavigationEvent.GoBack)
    }

    private fun Routes.destination() = EventBuilder(this.route)

    inner class EventBuilder(var dest: String) {
        private var popUpTo: NavigationEvent.PopUp? = null

        fun popUpTo(popUpTo: Routes) = apply {
            this.popUpTo = NavigationEvent.PopUp.To(popUpTo.route)
        }

        fun go() {
            _channel.trySend(NavigationEvent.GoTo(dest, popUpTo))
        }
    }
}


sealed class NavigationEvent {
    object GoBack : NavigationEvent()

    data class GoTo(val route: String, val popUpTo: PopUp? = null) : NavigationEvent()

    sealed class PopUp {
        data class To(val route: String, val inclusive: Boolean = true) : PopUp()
        object All : PopUp()
    }
}


sealed class Routes(val route: String) {
    object Splash : Routes("splash")
    object Dashboard : Routes("dashboard") {
        object Home : Routes("home/home")
        object Search : Routes("home/search")
        object Profile : Routes("home/profile")
    }

    object Login : Routes("login")
    object Player : Routes("player")
}