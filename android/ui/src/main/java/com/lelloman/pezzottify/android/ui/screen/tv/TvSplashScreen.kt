package com.lelloman.pezzottify.android.ui.screen.tv

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.screen.splash.SplashViewModel
import com.lelloman.pezzottify.android.ui.tv.fromTvSplashToLogin
import com.lelloman.pezzottify.android.ui.tv.fromTvSplashToNowPlaying

@Composable
fun TvSplashScreen(navController: NavController) {
    val viewModel = hiltViewModel<SplashViewModel>()

    LaunchedEffect(Unit) {
        viewModel.destination.collect { destination ->
            when (destination) {
                SplashViewModel.Destination.Main -> navController.fromTvSplashToNowPlaying()
                SplashViewModel.Destination.Login -> navController.fromTvSplashToLogin()
            }
        }
    }
}
