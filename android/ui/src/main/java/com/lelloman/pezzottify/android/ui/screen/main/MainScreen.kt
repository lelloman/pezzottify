package com.lelloman.pezzottify.android.ui.screen.main

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Home
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
    val route: Screen.Main,
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
    );

    val routeString: String = route::class.qualifiedName.orEmpty()
}

@Composable
fun MainScreen(parentNavController: NavController) {
    val navController = rememberNavController()
    Scaffold(
        bottomBar = {
            NavigationBar {
                val backStackEntry by navController.currentBackStackEntryAsState()
                val currentDestination = backStackEntry?.destination
                BottomNavigationRoute.entries.forEach {
                    val isSelected = currentDestination?.route == it.routeString
                    NavigationBarItem(
                        icon = {
                            Icon(
                                it.icon,
                                contentDescription = stringResource(it.description)
                            )
                        },
                        label = { Text(stringResource(it.description)) },
                        selected = isSelected,
                        onClick = {
                            navController.navigate(it.route) {
                                popUpTo(navController.graph.findStartDestination().id) {
                                    saveState = true
                                }
                                launchSingleTop = true
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
            modifier = Modifier.padding(bottom = innerPadding.calculateBottomPadding()),
        ) {
            composable<Screen.Main.Home> {
                HomeScreen(parentNavController = parentNavController)
            }
            composable<Screen.Main.Search> { SearchScreen(parentNavController) }
            composable<Screen.Main.Library> { LibraryScreen() }
        }
    }
}