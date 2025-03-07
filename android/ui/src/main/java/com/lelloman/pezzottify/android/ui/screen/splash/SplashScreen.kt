package com.lelloman.pezzottify.android.ui.screen.splash

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.fromSplashToLogin
import com.lelloman.pezzottify.android.ui.fromSplashToMain

@Composable
fun SplashScreen(navController: NavController) {
    val viewModel = hiltViewModel<SplashViewModel>()

    LaunchedEffect(Unit) {
        viewModel.destination.collect {
            when (it) {
                SplashViewModel.Destination.Main -> navController.fromSplashToMain()
                SplashViewModel.Destination.Login -> navController.fromSplashToLogin()
            }
        }
    }
}