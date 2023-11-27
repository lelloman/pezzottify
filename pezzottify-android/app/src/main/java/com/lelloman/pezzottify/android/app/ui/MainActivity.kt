package com.lelloman.pezzottify.android.app.ui

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.app.ui.dashboard.DashboardScreen
import com.lelloman.pezzottify.android.app.ui.login.LoginScreen
import com.lelloman.pezzottify.android.app.ui.player.PlayerScreen
import com.lelloman.pezzottify.android.app.ui.splash.SplashScreen
import com.lelloman.pezzottify.android.app.ui.theme.PezzottifyTheme
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.flow.receiveAsFlow
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject
    lateinit var navigator: Navigator

    @OptIn(ExperimentalMaterial3Api::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            val navController = rememberNavController()
            val navigationChannel = navigator.channel
            PezzottifyTheme {
                LaunchedEffect(key1 = this@MainActivity, navController, navigationChannel) {
                    navigator.channel.receiveAsFlow().collect { navigationEvent ->
                        when (navigationEvent) {
                            NavigationEvent.GoBack -> navController.popBackStack()
                            is NavigationEvent.GoTo -> {
                                navController.navigate(navigationEvent.route) {
                                    navigationEvent.popUpTo?.let { popUpDef ->
                                        popUpTo(popUpDef.route) {
                                            inclusive = popUpDef.inclusive
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Scaffold { _ ->
                    PezzottifyNavHost(navController = navController)
                }
            }
        }
    }
}

@Composable
fun PezzottifyNavHost(navController: NavHostController) {
    NavHost(
        modifier = Modifier.fillMaxSize(),
        navController = navController,
        startDestination = Routes.Splash.route,
        enterTransition = { EnterTransition.None },
        popEnterTransition = { EnterTransition.None },
        exitTransition = { ExitTransition.None },
        popExitTransition = { ExitTransition.None },
    ) {
        composable(Routes.Splash.route) { SplashScreen() }
        composable(Routes.Login.route) { LoginScreen() }
        composable(Routes.Dashboard.route) { DashboardScreen() }
        composable(Routes.Player.route) { PlayerScreen() }
    }
}

@Preview(showBackground = true)
@Composable
fun DefaultPreview() {
    PezzottifyTheme {
        ScreenMain()
    }
}

@Composable
fun ScreenMain() {
    val navController = rememberNavController()
    NavHost(navController = navController, startDestination = Routes.Login.route) {
        composable(Routes.Login.route) {
            LoginScreen()
        }
    }
}