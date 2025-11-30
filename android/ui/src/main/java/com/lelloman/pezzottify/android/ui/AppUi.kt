package com.lelloman.pezzottify.android.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.screen.about.AboutScreen
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreen
import com.lelloman.pezzottify.android.ui.screen.main.MainScreen
import com.lelloman.pezzottify.android.ui.screen.splash.SplashScreen
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun AppUi(darkTheme: Boolean = isSystemInDarkTheme()) {
    val navController = rememberNavController()
    PezzottifyTheme(darkTheme = darkTheme) {
        Box(modifier = Modifier.fillMaxSize()) {
            NavHost(
                modifier = Modifier.fillMaxSize(),
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
                    MainScreen(rootNavController = navController)
                }
            }
        }
    }
}

@Preview
@Composable
fun AppUiPreview() {
    AppUi()
}
