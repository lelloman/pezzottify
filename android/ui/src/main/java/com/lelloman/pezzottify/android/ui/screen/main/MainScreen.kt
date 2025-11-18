package com.lelloman.pezzottify.android.ui.screen.main

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.Screen
import com.lelloman.pezzottify.android.ui.screen.main.content.album.AlbumScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.artist.ArtistScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.track.TrackScreen
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreen
import com.lelloman.pezzottify.android.ui.screen.main.library.LibraryScreen
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreen
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
fun MainScreen() {
    val viewModel = hiltViewModel<MainScreenViewModel>()
    MainScreenContent(state = viewModel.state.collectAsState().value, viewModel)
}

@Composable
private fun MainScreenContent(state: MainScreenState, actions: MainScreenActions) {
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
                                popUpTo(Screen.Main.Home) {
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
        Column(modifier = Modifier.padding(bottom = innerPadding.calculateBottomPadding())) {

            NavHost(
                modifier = Modifier.weight(1f),
                navController = navController,
                startDestination = Screen.Main.Home,
            ) {
                composable<Screen.Main.Home> {
                    HomeScreen(navController = navController)
                }
                composable<Screen.Main.Search> { SearchScreen(navController) }
                composable<Screen.Main.Library> { LibraryScreen() }

                composable<Screen.Main.Home.Profile> {
                    ProfileScreen(navController)
                }
                composable<Screen.Main.Artist> {
                    ArtistScreen(it.toRoute<Screen.Main.Artist>().artistId)
                }
                composable<Screen.Main.Album> {
                    AlbumScreen(it.toRoute<Screen.Main.Album>().albumId)
                }
                composable<Screen.Main.Track> {
                    TrackScreen(it.toRoute<Screen.Main.Track>().trackId)
                }
            }
            if (state.bottomPlayer.isVisible) {
                Row(
                    modifier = Modifier
                        .height(64.dp)
                        .fillMaxWidth()
                        .background(color = Color.Red)
                ) {
                    Text(state.bottomPlayer.trackName, modifier = Modifier.weight(1f))
                    IconButton(onClick = actions::clickOnSkipToPrevious) {
                        Icon(
                            modifier = Modifier.size(48.dp),
                            painter = painterResource(R.drawable.baseline_skip_previous_24),
                            contentDescription = null,
                        )
                    }
                    IconButton(onClick = actions::clickOnPlayPause) {
                        Icon(
                            modifier = Modifier.size(48.dp),
                            painter = painterResource(if (state.bottomPlayer.isPlaying) R.drawable.baseline_pause_circle_24 else R.drawable.baseline_play_circle_24),
                            contentDescription = null,
                        )
                    }
                    IconButton(onClick = actions::clickOnSkipToNext) {
                        Icon(
                            modifier = Modifier.size(48.dp),
                            painter = painterResource(R.drawable.baseline_skip_next_24),
                            contentDescription = null,
                        )
                    }
                }
            }
        }
    }
}