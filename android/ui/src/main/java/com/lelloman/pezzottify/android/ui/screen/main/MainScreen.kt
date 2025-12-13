package com.lelloman.pezzottify.android.ui.screen.main

import androidx.activity.compose.BackHandler
import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.Screen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.component.ScrollingTextRow
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.screen.main.content.album.AlbumScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.artist.ArtistScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.ExternalAlbumScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.fullscreenimage.FullScreenImageScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.track.TrackScreen
import com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist.UserPlaylistScreen
import com.lelloman.pezzottify.android.ui.screen.player.PlayerScreen
import com.lelloman.pezzottify.android.ui.screen.queue.QueueScreen
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toExternalAlbum
import com.lelloman.pezzottify.android.ui.toPlayer
import com.lelloman.pezzottify.android.ui.toProfile
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreen
import com.lelloman.pezzottify.android.ui.screen.main.library.LibraryScreen
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileDrawerContent
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreen
import com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings.StyleSettingsScreen
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreen
import com.lelloman.pezzottify.android.ui.screen.main.settings.SettingsScreen
import com.lelloman.pezzottify.android.ui.screen.main.settings.logviewer.LogViewerScreen

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
fun MainScreen(rootNavController: androidx.navigation.NavController) {
    val viewModel = hiltViewModel<MainScreenViewModel>()
    MainScreenContent(state = viewModel.state.collectAsState().value, viewModel, rootNavController)
}

// Routes where bottom navigation and bottom player should be hidden
private val overlayRoutes = setOf(
    Screen.Main.Player::class.qualifiedName,
    Screen.Main.Queue::class.qualifiedName,
    Screen.Main.FullScreenImage::class.qualifiedName,
    Screen.Main.Home.LogViewer::class.qualifiedName,
)

@Composable
private fun MainScreenContent(state: MainScreenState, actions: MainScreenActions, rootNavController: androidx.navigation.NavController) {
    val navController = rememberNavController()
    val backStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = backStackEntry?.destination?.route

    // Hide bottom bars for overlay screens (Player, Queue, FullScreenImage)
    val isOverlayScreen = overlayRoutes.any { currentRoute?.startsWith(it ?: "") == true }

    // Drawer state for profile drawer
    val drawerState = rememberDrawerState(initialValue = DrawerValue.Closed)
    val drawerScope = rememberCoroutineScope()
    var shouldRestoreDrawer by rememberSaveable { mutableStateOf(false) }

    // Restore drawer when navigating back to Home
    LaunchedEffect(currentRoute) {
        val isOnHome = currentRoute == Screen.Main.Home::class.qualifiedName
        if (isOnHome && shouldRestoreDrawer) {
            drawerState.open()
            shouldRestoreDrawer = false
        }
    }

    // Close drawer on back press instead of exiting the app
    BackHandler(enabled = drawerState.isOpen) {
        drawerScope.launch { drawerState.close() }
    }

    ModalNavigationDrawer(
        drawerState = drawerState,
        gesturesEnabled = drawerState.isOpen,
        drawerContent = {
            ProfileDrawerContent(
                onNavigateToProfile = {
                    shouldRestoreDrawer = true
                    navController.toProfile()
                },
                onNavigateToMyRequests = {
                    shouldRestoreDrawer = true
                    navController.navigate(Screen.Main.MyRequests)
                },
                onNavigateToListeningHistory = {
                    shouldRestoreDrawer = true
                    navController.navigate(Screen.Main.ListeningHistory)
                },
                onNavigateToAbout = {
                    // Navigate to About screen (root nav) - don't restore drawer
                    rootNavController.navigate(Screen.About)
                },
                onNavigateToLogin = {
                    rootNavController.navigate(Screen.Login) {
                        popUpTo(Screen.Main.Home) { inclusive = true }
                    }
                },
                onCloseDrawer = {
                    drawerScope.launch { drawerState.close() }
                },
            )
        }
    ) {
    Scaffold(
        bottomBar = {
            if (!isOverlayScreen) {
                NavigationBar {
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
                                // Try to pop to this tab's root first (handles re-clicking current tab)
                                val popped = navController.popBackStack(it.route, inclusive = false)
                                if (!popped) {
                                    // Tab not in back stack - navigate to it
                                    navController.navigate(it.route) {
                                        popUpTo(Screen.Main.Home) {
                                            saveState = true
                                        }
                                        launchSingleTop = true
                                        restoreState = true
                                    }
                                }
                            }
                        )
                    }
                }
            }
        }
    ) { innerPadding ->
        Column(modifier = Modifier.padding(bottom = if (isOverlayScreen) 0.dp else innerPadding.calculateBottomPadding())) {

            NavHost(
                modifier = Modifier.weight(1f),
                navController = navController,
                startDestination = Screen.Main.Home,
            ) {
                composable<Screen.Main.Home> {
                    HomeScreen(
                        navController = navController,
                        onOpenProfileDrawer = { drawerScope.launch { drawerState.open() } },
                    )
                }
                composable<Screen.Main.Search> { SearchScreen(navController) }
                composable<Screen.Main.Library> { LibraryScreen(navController) }

                composable<Screen.Main.Home.Profile> {
                    ProfileScreen(navController, rootNavController)
                }
                composable<Screen.Main.Home.Settings> {
                    SettingsScreen(navController)
                }
                composable<Screen.Main.Home.StyleSettings> {
                    StyleSettingsScreen(navController)
                }
                composable<Screen.Main.Home.LogViewer> {
                    LogViewerScreen(navController)
                }
                composable<Screen.Main.Artist> {
                    ArtistScreen(it.toRoute<Screen.Main.Artist>().artistId, navController)
                }
                composable<Screen.Main.Album> {
                    AlbumScreen(it.toRoute<Screen.Main.Album>().albumId, navController)
                }
                composable<Screen.Main.Track> {
                    TrackScreen(it.toRoute<Screen.Main.Track>().trackId, navController)
                }
                composable<Screen.Main.UserPlaylist> {
                    UserPlaylistScreen(it.toRoute<Screen.Main.UserPlaylist>().playlistId, navController)
                }
                composable<Screen.Main.MyRequests> {
                    com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreen(
                        onNavigateBack = { navController.popBackStack() },
                        onNavigateToAlbum = { albumId -> navController.toAlbum(albumId) },
                        onNavigateToExternalAlbum = { albumId -> navController.toExternalAlbum(albumId) },
                    )
                }
                composable<Screen.Main.ListeningHistory> {
                    com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.ListeningHistoryScreen(
                        onNavigateBack = { navController.popBackStack() },
                        onNavigateToTrack = { trackId -> navController.navigate(Screen.Main.Track(trackId)) },
                    )
                }
                composable<Screen.Main.ExternalAlbum> {
                    ExternalAlbumScreen(
                        albumId = it.toRoute<Screen.Main.ExternalAlbum>().albumId,
                        navController = navController
                    )
                }

                // Overlay screens (no bottom nav/player)
                composable<Screen.Main.Player> {
                    PlayerScreen(navController = navController)
                }
                composable<Screen.Main.Queue> {
                    QueueScreen(navController = navController)
                }
                composable<Screen.Main.FullScreenImage> {
                    FullScreenImageScreen(
                        imageUrl = it.toRoute<Screen.Main.FullScreenImage>().imageUrl,
                        navController = navController
                    )
                }
            }
            if (state.bottomPlayer.isVisible && !isOverlayScreen) {
                BottomPlayer(state.bottomPlayer, actions, onClickPlayer = { navController.toPlayer() })
            }
        }
    }
    } // End ModalNavigationDrawer
}

private data class PagerTrackInfo(
    val trackName: String,
    val artists: List<ArtistInfo>,
)

@Composable
private fun TrackInfoPage(
    trackName: String,
    artists: List<ArtistInfo>,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier.padding(end = 12.dp),
        verticalArrangement = androidx.compose.foundation.layout.Arrangement.Center,
    ) {
        ScrollingTextRow(
            text = trackName,
            textStyle = MaterialTheme.typography.bodyMedium,
            textColor = MaterialTheme.colorScheme.onSurface,
        )
        ScrollingArtistsRow(
            artists = artists,
            textStyle = MaterialTheme.typography.bodySmall,
            textColor = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun BottomPlayer(state: MainScreenState.BottomPlayer, actions: MainScreenActions, onClickPlayer: () -> Unit) {
    // Build the list of tracks for the pager: [previous?, current, next?]
    val hasPrevious = state.previousTrackName != null
    val hasNext = state.nextTrackName != null

    val pagerTracks = remember(
        state.trackName,
        state.artists,
        state.previousTrackName,
        state.previousTrackArtists,
        state.nextTrackName,
        state.nextTrackArtists
    ) {
        buildList {
            if (state.previousTrackName != null) {
                add(PagerTrackInfo(state.previousTrackName, state.previousTrackArtists))
            }
            add(PagerTrackInfo(state.trackName, state.artists))
            if (state.nextTrackName != null) {
                add(PagerTrackInfo(state.nextTrackName, state.nextTrackArtists))
            }
        }
    }

    // Current track is at index 0 if no previous, or index 1 if there's a previous track
    val currentPageIndex = if (hasPrevious) 1 else 0

    // Use trackId + hasPrevious as key to recreate pager when track or structure changes
    val pagerState = key(state.trackId, hasPrevious) {
        rememberPagerState(
            initialPage = currentPageIndex,
            pageCount = { pagerTracks.size }
        )
    }

    // Handle swipe gestures - trigger skip when user settles on a different page
    LaunchedEffect(pagerState, currentPageIndex) {
        snapshotFlow { pagerState.settledPage }
            .collectLatest { settledPage ->
                when {
                    settledPage < currentPageIndex -> actions.clickOnSkipToPrevious()
                    settledPage > currentPageIndex -> actions.clickOnSkipToNext()
                }
            }
    }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceContainer)
    ) {
        Column {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .height(56.dp)
                    .fillMaxWidth()
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .weight(1f)
                        .clickable(onClick = onClickPlayer)
                ) {
                    NullablePezzottifyImage(
                        url = state.albumImageUrl,
                        shape = PezzottifyImageShape.MiniPlayer,
                        modifier = Modifier.clip(RoundedCornerShape(4.dp))
                    )

                    HorizontalPager(
                        state = pagerState,
                        modifier = Modifier
                            .weight(1f)
                            .padding(horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) { page ->
                        val trackInfo = pagerTracks.getOrNull(page)
                        if (trackInfo != null) {
                            TrackInfoPage(
                                trackName = trackInfo.trackName,
                                artists = trackInfo.artists,
                                modifier = Modifier.fillMaxSize()
                            )
                        }
                    }
                }

                IconButton(onClick = actions::clickOnPlayPause) {
                    Icon(
                        modifier = Modifier.size(48.dp),
                        painter = painterResource(if (state.isPlaying) R.drawable.baseline_pause_24 else R.drawable.baseline_play_arrow_24),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurface,
                    )
                }
            }

            LinearProgressIndicator(
                progress = { state.trackPercent / 100f },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(2.dp),
                color = MaterialTheme.colorScheme.primary,
                trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
            )
        }
    }
}

@Preview
@Composable
private fun PreviewBottomPlayer() {
    BottomPlayer(
        state = MainScreenState.BottomPlayer(
            isVisible = true,
            trackName = "A very long track name to see what happens when it is very very long",
            albumName = "Album Name",
            albumImageUrl = null,
            artists = listOf(
                ArtistInfo("1", "Artist One"),
                ArtistInfo("2", "Artist Two"),
            ),
            isPlaying = true,
            trackPercent = 35f,
            nextTrackName = "Next Track Name",
            nextTrackArtists = listOf(ArtistInfo("3", "Next Artist")),
            previousTrackName = "Previous Track Name",
            previousTrackArtists = listOf(ArtistInfo("4", "Previous Artist")),
        ),
        actions = object : MainScreenActions {
            override fun clickOnPlayPause() = Unit
            override fun clickOnSkipToNext() = Unit
            override fun clickOnSkipToPrevious() = Unit
        },
        onClickPlayer = {}
    )
}
