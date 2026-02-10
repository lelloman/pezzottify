package com.lelloman.pezzottify.android.ui.tv

import android.widget.Toast
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.SessionExpiredViewModel
import com.lelloman.pezzottify.android.ui.screen.tv.TvLoginScreen
import com.lelloman.pezzottify.android.ui.screen.tv.TvNowPlayingScreen
import com.lelloman.pezzottify.android.ui.screen.tv.TvSettingsScreen
import com.lelloman.pezzottify.android.ui.screen.tv.TvSplashScreen
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import com.lelloman.pezzottify.android.ui.theme.ThemeMode

@Composable
fun TvAppUi(
    darkTheme: Boolean,
    themeMode: ThemeMode,
    colorPalette: ColorPalette,
    fontFamily: AppFontFamily,
) {
    val navController = rememberNavController()
    PezzottifyTheme(
        darkTheme = darkTheme,
        themeMode = themeMode,
        colorPalette = colorPalette,
        fontFamily = fontFamily,
    ) {
        Box(modifier = Modifier.fillMaxSize()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(androidx.compose.material3.MaterialTheme.colorScheme.background)
            )
            TvSessionExpiredEffect(navController)

            NavHost(
                modifier = Modifier.fillMaxSize(),
                navController = navController,
                startDestination = TvScreen.Splash,
            ) {
                composable<TvScreen.Splash> {
                    TvSplashScreen(navController)
                }
                composable<TvScreen.Login> {
                    TvLoginScreen(navController)
                }
                composable<TvScreen.NowPlaying> {
                    TvNowPlayingScreen(
                        onOpenSettings = { navController.fromTvNowPlayingToSettings() },
                    )
                }
                composable<TvScreen.Settings> {
                    TvSettingsScreen(navController)
                }
            }
        }
    }
}

@Composable
private fun TvSessionExpiredEffect(navController: NavController) {
    val viewModel = hiltViewModel<SessionExpiredViewModel>()
    val context = LocalContext.current

    LaunchedEffect(Unit) {
        viewModel.sessionExpiredEvents.collect {
            Toast.makeText(
                context,
                "Session expired. Please log in again.",
                Toast.LENGTH_LONG
            ).show()
            viewModel.handleSessionExpired()
            navController.fromTvNowPlayingToLogin()
        }
    }
}
