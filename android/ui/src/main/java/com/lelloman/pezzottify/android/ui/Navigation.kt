package com.lelloman.pezzottify.android.ui

import androidx.navigation.NavController
import androidx.navigation.PopUpToBuilder
import com.lelloman.pezzottify.android.ui.Screen.Login
import com.lelloman.pezzottify.android.ui.Screen.Main
import com.lelloman.pezzottify.android.ui.Screen.Splash
import kotlinx.serialization.Serializable

private fun inclusive(): PopUpToBuilder.() -> Unit = { inclusive = true }

fun NavController.fromSplashToMain() = navigate(Main.Home) {
    popUpTo(Splash, inclusive())
}

fun NavController.fromSplashToLogin() = navigate(Login) {
    popUpTo(Splash, inclusive())
}

fun NavController.fromLoginToMain() = navigate(Main.Home) {
    popUpTo(Login, inclusive())
}

fun NavController.fromMainBackToLogin() = navigate(Login) {
    popUpTo(Main.Home, inclusive())
}

fun NavController.toProfile() = navigate(Main.Home.Profile)

fun NavController.fromProfileBackToLogin() = navigate(Login) {
    popUpTo(Main.Home, inclusive())
}

fun NavController.toArtist(artistId: String) = navigate(Main.Artist(artistId))

fun NavController.toTrack(trackId: String) = navigate(Main.Track(trackId))

fun NavController.toAlbum(albumId: String) = navigate(Main.Album(albumId))

sealed interface Screen {

    @Serializable
    data object Splash : Screen

    @Serializable
    data object Login : Screen

    @Serializable
    data object About : Screen

    sealed interface Main : Screen {

        @Serializable
        data object Home : Main {

            @Serializable
            data object Profile : Main
        }

        @Serializable
        data object Search : Main

        @Serializable
        data object Library : Main

        @Serializable
        data class Artist(val artistId: String) : Main

        @Serializable
        data class Track(val trackId: String) : Main

        @Serializable
        data class Album(val albumId: String) : Main

        @Serializable
        data class FullScreenImage(val imageUrls: String) : Main

        @Serializable
        data object Player : Main

        @Serializable
        data object Queue : Main
    }
}

fun NavController.toFullScreenImage(imageUrls: List<String>) {
    val encoded = imageUrls.joinToString(separator = "|")
    navigate(Screen.Main.FullScreenImage(encoded))
}

fun NavController.toPlayer() = navigate(Screen.Main.Player)

fun NavController.toQueue() = navigate(Screen.Main.Queue)