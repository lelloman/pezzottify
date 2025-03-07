package com.lelloman.pezzottify.android.ui.screen.main

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.List
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.navigation.NavController
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.Screen
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreen
import com.lelloman.pezzottify.android.ui.screen.main.library.LibraryScreen
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreen

enum class BottomNavigationRoute(
    val route: Screen,
    val icon: ImageVector,
    @StringRes val description: Int,
) {
    Home(
        route = Screen.Main.Home,
        icon = Icons.Filled.Home,
        description = R.string.home_navigation_item_description,
    ),
    Search(
        route = Screen.Main.Search,
        icon = Icons.Filled.Search,
        description = R.string.search_navigation_item_description,
    ),
    Library(
        route = Screen.Main.Library,
        icon = Icons.Filled.Menu,
        description = R.string.library_navigation_item_description,
    ),
}

@Composable
fun MainScreen(parentNavController: NavController) {
    val navController = rememberNavController()
    Scaffold(
        bottomBar = {
            NavigationBar {
                val navBackStackEntry by navController.currentBackStackEntryAsState()
                val currentDestination = navBackStackEntry?.destination
                BottomNavigationRoute.entries.forEach {
                    NavigationBarItem(
                        icon = {
                            Icon(
                                it.icon,
                                contentDescription = stringResource(it.description)
                            )
                        },
                        label = { Text(stringResource(it.description)) },
                        selected = currentDestination?.route == it.route.toString(),
                        onClick = {
                            navController.navigate(it.route) {
                                // Pop up to the start destination of the graph to
                                // avoid building up a large stack of destinations
                                // on the back stack as users select items
                                popUpTo(navController.graph.findStartDestination().id) {
                                    saveState = true
                                }
                                // Avoid multiple copies of the same destination when
                                // reselecting the same item
                                launchSingleTop = true
                                // Restore state when reselecting a previously selected item
                                restoreState = true
                            }
                        }
                    )
                }
            }
        }
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = Screen.Main.Home,
            modifier = Modifier.padding(innerPadding),
        ) {
            composable<Screen.Main.Home> { HomeScreen(Modifier, parentNavController) }
            composable<Screen.Main.Search> { SearchScreen() }
            composable<Screen.Main.Library> { LibraryScreen() }
        }
    }
}