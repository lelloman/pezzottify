package com.lelloman.pezzottify.android.app.ui.dashboard

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
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
fun DashboardGraph(navHostController: NavHostController, paddingValues: PaddingValues) {
    NavHost(
        navController = navHostController,
        route = Routes.Dashboard.route,
        startDestination = Routes.Dashboard.Home.route,
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
) {
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
        DashboardGraph(navController, paddingValues)
    }
}