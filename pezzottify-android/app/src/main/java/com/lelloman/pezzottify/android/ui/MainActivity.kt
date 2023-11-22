package com.lelloman.pezzottify.android.ui

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.tooling.preview.Preview
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.home.HomePage
import com.lelloman.pezzottify.android.ui.login.LoginPage
import com.lelloman.pezzottify.android.ui.splash.SplashPage
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.flow.receiveAsFlow
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject
    lateinit var navigator: Navigator

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
                PezzottifyNavHost(navController = navController)
            }
        }
    }
}

@Composable
fun PezzottifyNavHost(navController: NavHostController) {
    NavHost(navController = navController, startDestination = "splash") {
        composable(Routes.Splash.route) { SplashPage() }
        composable(Routes.Login.route) { LoginPage() }
        composable(Routes.Home.route) { HomePage() }
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
            LoginPage()
        }
    }
}