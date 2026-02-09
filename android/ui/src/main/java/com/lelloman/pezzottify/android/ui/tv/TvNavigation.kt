package com.lelloman.pezzottify.android.ui.tv

import androidx.navigation.NavController
import androidx.navigation.PopUpToBuilder
import kotlinx.serialization.Serializable

private fun inclusive(): PopUpToBuilder.() -> Unit = { inclusive = true }

fun NavController.fromTvSplashToLogin() = navigate(TvScreen.Login) {
    popUpTo(TvScreen.Splash, inclusive())
}

fun NavController.fromTvSplashToNowPlaying() = navigate(TvScreen.NowPlaying) {
    popUpTo(TvScreen.Splash, inclusive())
}

fun NavController.fromTvLoginToNowPlaying() = navigate(TvScreen.NowPlaying) {
    popUpTo(TvScreen.Login, inclusive())
}

fun NavController.fromTvNowPlayingToLogin() = navigate(TvScreen.Login) {
    popUpTo(TvScreen.NowPlaying, inclusive())
}

sealed interface TvScreen {

    @Serializable
    data object Splash : TvScreen

    @Serializable
    data object Login : TvScreen

    @Serializable
    data object NowPlaying : TvScreen
}
