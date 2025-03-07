package com.lelloman.pezzottify.android.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.screen.about.AboutScreen
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreen
import com.lelloman.pezzottify.android.ui.screen.main.MainScreen
import com.lelloman.pezzottify.android.ui.screen.splash.SplashScreen

@Composable
fun AppUi() {
    val navController = rememberNavController()
    NavHost(
        navController = navController,
        startDestination = Screen.Splash,
    ) {
        composable<Screen.Splash> {
            SplashScreen()
        }
        composable<Screen.Login> {
            LoginScreen()
        }
        composable<Screen.About> {
            AboutScreen()
        }
        composable<Screen.Main> {
            MainScreen()
        }
    }
}

@Preview
@Composable
fun AppUiPreview() {
    AppUi()
}