package com.lelloman.pezzottify.android.app.ui.dashboard

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.app.R
import com.lelloman.pezzottify.android.app.ui.Routes
import com.lelloman.pezzottify.android.app.ui.dashboard.home.HomeScreen
import com.lelloman.pezzottify.android.app.ui.dashboard.profile.ProfileScreen
import com.lelloman.pezzottify.android.app.ui.dashboard.search.SearchScreen


data class BottomNavigationItem(
    val label: String = "", val icon: ImageVector = Icons.Filled.Home, val route: String = ""
) {
    fun bottomNavigationItems(): List<BottomNavigationItem> {
        return listOf(
            BottomNavigationItem(
                label = "Home",
                icon = Icons.Filled.Home,
                route = Routes.Dashboard.Home.route,
            ),
            BottomNavigationItem(
                label = "Search", icon = Icons.Filled.Search, route = Routes.Dashboard.Search.route
            ),
            BottomNavigationItem(
                label = "Profile",
                icon = Icons.Filled.AccountCircle,
                route = Routes.Dashboard.Profile.route
            ),
        )
    }
}

@Composable
fun DashboardGraph(navHostController: NavHostController, modifier: Modifier) {
    NavHost(
        modifier = modifier,
        navController = navHostController,
        route = Routes.Dashboard.route,
        startDestination = Routes.Dashboard.Home.route,
        enterTransition = { EnterTransition.None },
        popEnterTransition = { EnterTransition.None },
        exitTransition = { ExitTransition.None },
        popExitTransition = { ExitTransition.None },
    ) {
        composable(Routes.Dashboard.Home.route) { HomeScreen() }
        composable(Routes.Dashboard.Search.route) { SearchScreen() }
        composable(Routes.Dashboard.Profile.route) { ProfileScreen() }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DashboardScreen(
    navController: NavHostController = rememberNavController(),
    viewModel: DashboardViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsState()
    Scaffold(modifier = Modifier.fillMaxSize(), bottomBar = {
        NavigationBar {
            val navBackStackEntry by navController.currentBackStackEntryAsState()
            val currentRoute = navBackStackEntry?.destination?.route
            BottomNavigationItem().bottomNavigationItems().forEach { navigationItem ->
                NavigationBarItem(selected = currentRoute == navigationItem.route, label = {
                    Text(navigationItem.label)
                }, icon = {
                    Icon(
                        navigationItem.icon, contentDescription = navigationItem.label
                    )
                }, onClick = {
                    navController.navigate(navigationItem.route) {
                        popUpTo(navController.graph.findStartDestination().id) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                })
            }
        }
    }) { paddingValues ->
        Column(
            modifier = Modifier.padding(paddingValues)
        ) {
            DashboardGraph(navController, Modifier.weight(1f, true))
            AnimatedVisibility(visible = state.playerControlsState != null) {
                PlayerControls(
                    Modifier,
                    state.playerControlsState ?: DashboardViewModel.PlayerControlsState(),
                    onPlayPauseButtonClicked = viewModel::onPlayPauseButtonClicked,
                )
            }
        }
    }
}

@Composable
fun PlayerControls(
    modifier: Modifier,
    playerControlsState: DashboardViewModel.PlayerControlsState,
    onPlayPauseButtonClicked: () -> Unit,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(color = Color(0xffbbbbbb))
            .padding(8.dp)
    ) {
        var sliderPosition by remember { mutableFloatStateOf(0f) }
        Row(horizontalArrangement = Arrangement.Center, modifier = Modifier.fillMaxWidth()) {
            IconButton(modifier = Modifier.size(48.dp), onClick = {}) {
                Icon(
                    painter = painterResource(R.drawable.skip_previous_24),
                    contentDescription = "play/pause",
                    modifier = Modifier.fillMaxSize()
                )
            }
            Spacer(modifier = Modifier.width(24.dp))
            IconButton(modifier = Modifier.size(48.dp), onClick = onPlayPauseButtonClicked) {
                val iconResource =
                    if (playerControlsState.isPlaying) R.drawable.pause_circle_outline_24 else R.drawable.play_circle_24
                Icon(
                    painter = painterResource(iconResource),
                    contentDescription = "play/pause",
                    modifier = Modifier.fillMaxSize()
                )
            }
            Spacer(modifier = Modifier.width(24.dp))
            IconButton(modifier = Modifier.size(48.dp), onClick = {}) {
                Icon(
                    painter = painterResource(R.drawable.skip_next_24),
                    contentDescription = "play/pause",
                    modifier = Modifier.fillMaxSize()
                )
            }
        }
        Canvas(
            modifier = Modifier
                .fillMaxWidth()
                .height(4.dp),
        ) {
            val start = Offset(0f, size.height / 2)
            val end = Offset(size.width * playerControlsState.trackPercent, size.height / 2)
            drawLine(color = Color.Black, start = start, end = end)
        }
    }
}

@Preview
@Composable
fun PlayerControlsPreview() {
    Box(contentAlignment = Alignment.Center, modifier = Modifier.fillMaxSize()) {
        PlayerControls(
            modifier = Modifier.defaultMinSize(),
            DashboardViewModel.PlayerControlsState(),
            onPlayPauseButtonClicked = {},
        )
    }
}