package com.lelloman.pezzottify.android.ui.screen.main.home

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.fromMainBackToLogin
import com.lelloman.pezzottify.android.ui.toProfile
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch


@Composable
fun HomeScreen(parentNavController: NavController) {
    val viewModel = hiltViewModel<HomeScreenViewModel>()
    HomeScreenContent(
        parentNavController = parentNavController,
        actions = viewModel,
        events = viewModel.events
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HomeScreenContent(
    parentNavController: NavController,
    events: Flow<HomeScreenEvents>,
    actions: HomeScreenActions,
) {
    val coroutineScope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                HomeScreenEvents.NavigateToProfileScreen -> {
                    parentNavController.toProfile()
                }
            }
        }
    }

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        topBar = {
            TopAppBar(
                title = { Text("Home") },
                actions = {
                    IconButton(onClick = {
                        coroutineScope.launch { actions.clickOnProfile() }
                    }) {
                        Icon(
                            imageVector = Icons.Default.Person,
                            contentDescription = null,
                            tint = MaterialTheme.typography.headlineLarge.color
                        )
                    }
                }
            )
        }
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .padding(innerPadding)
                .fillMaxSize()
        ) {
            Text(
                "HOME",
                modifier = Modifier.align(Alignment.Center),
                style = MaterialTheme.typography.headlineLarge
            )
            Button(
                modifier = Modifier.align(Alignment.BottomCenter),
                onClick = { parentNavController.fromMainBackToLogin() }) {
                Text("LOGOUT")
            }
        }
    }
}

@Composable
@Preview
private fun HomeScreenPreview() {
    val navController = rememberNavController()
    HomeScreen(parentNavController = navController)
}