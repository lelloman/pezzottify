package com.lelloman.pezzottify.android.ui.screen.main.profile

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.fromProfileBackToLogin
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow

@Composable
fun ProfileScreen(navController: NavController) {
    val viewModel = hiltViewModel<ProfileScreenViewModel>()
    ProfileScreenInternal(
        events = viewModel.events,
        navController = navController,
        actions = viewModel,
    )
}

@Composable
private fun ProfileScreenInternal(
    actions: ProfileScreenActions,
    events: Flow<ProfileScreenEvents>,
    navController: NavController,
) {
    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                ProfileScreenEvents.NavigateToLoginScreen -> {
                    navController.fromProfileBackToLogin()
                }
            }
        }
    }
    Box(modifier = Modifier.fillMaxSize()) {
        Text(
            "PROFILE",
            modifier = Modifier.align(Alignment.Center),
            style = MaterialTheme.typography.headlineLarge
        )
        Button(
            modifier = Modifier.align(Alignment.BottomCenter),
            onClick = actions::clickOnLogout
        ) {
            Text("LOGOUT")
        }
    }
}

@Composable
@Preview
private fun ProfileScreenPreview() {
    ProfileScreenInternal(
        events = flow {},
        navController = rememberNavController(),
        actions = object : ProfileScreenActions {
            override fun clickOnLogout() {

            }
        },
    )
}