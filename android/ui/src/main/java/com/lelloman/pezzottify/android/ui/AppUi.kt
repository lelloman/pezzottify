package com.lelloman.pezzottify.android.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import com.lelloman.pezzottify.android.ui.screen.about.AboutScreen
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreen
import com.lelloman.pezzottify.android.ui.screen.main.MainScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.album.AlbumScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.artist.ArtistScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.track.TrackScreen
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreen
import com.lelloman.pezzottify.android.ui.screen.splash.SplashScreen
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun AppUi() {
    val navController = rememberNavController()
    PezzottifyTheme {
        NavHost(
            navController = navController,
            startDestination = Screen.Splash,
        ) {
            composable<Screen.Splash> {
                SplashScreen(navController)
            }
            composable<Screen.Login> {
                LoginScreen(navController)
            }
            composable<Screen.About> {
                AboutScreen()
            }
            composable<Screen.Main.Home> {
                MainScreen(navController)
            }
            composable<Screen.Main.Home.Profile> {
                ProfileScreen(navController)
            }
            composable<Screen.Main.Artist> {
                ArtistScreen(it.toRoute<Screen.Main.Artist>().artistId)
            }

            composable<Screen.Main.Album> {
                AlbumScreen(it.toRoute<Screen.Main.Album>().albumId)
            }

            composable<Screen.Main.Track> {
                TrackScreen(it.toRoute<Screen.Main.Track>().trackId)
            }
        }
    }
}

@Preview
@Composable
fun AppUiPreview() {
    AppUi()
}