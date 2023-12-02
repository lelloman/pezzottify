package com.lelloman.pezzottify.android.app.ui

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.core.content.ContextCompat
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
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.receiveAsFlow
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject
    lateinit var navigator: Navigator

    @Inject
    lateinit var snackBarController: SnackBarController

    @OptIn(ExperimentalMaterial3Api::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (ContextCompat.checkSelfPermission(
                this,
                Manifest.permission.FOREGROUND_SERVICE
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.FOREGROUND_SERVICE), 123)
        }
        if (ContextCompat.checkSelfPermission(
                this,
                Manifest.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK), 123)
        }

        setContent {
            val navController = rememberNavController()
            val navigationChannel = navigator.channel
            val snackbarHostState = remember { SnackbarHostState() }
            PezzottifyTheme {
                LaunchedEffect(key1 = this@MainActivity, navController, navigationChannel) {
                    navigator.channel.receiveAsFlow().collect { navigationEvent ->
                        when (navigationEvent) {
                            NavigationEvent.GoBack -> navController.popBackStack()
                            is NavigationEvent.GoTo -> {
                                navController.navigate(navigationEvent.route) {
                                    navigationEvent.popUpTo?.let { popUpDef ->
                                        when (popUpDef) {
                                            is NavigationEvent.PopUp.To -> popUpTo(popUpDef.route) {
                                                inclusive = popUpDef.inclusive
                                            }

                                            is NavigationEvent.PopUp.All -> popUpTo(navController.graph.id) {
                                                inclusive = true
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Scaffold(
                    snackbarHost = {
                        SnackbarHost(hostState = snackbarHostState)
                    }
                ) { _ ->
                    LaunchedEffect(Unit) {
                        snackBarController.snacks.collectLatest {
                            if (it != null) {
                                snackbarHostState.showSnackbar(it.message)
                            }
                        }
                    }
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