package com.lelloman.pezzottify.android.ui.screen.main.home

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.BackOnlineBanner
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.OfflineIndicator
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.content.ArtistInfo
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.theme.ComponentSize
import com.lelloman.pezzottify.android.ui.theme.CornerRadius
import com.lelloman.pezzottify.android.ui.theme.Elevation
import com.lelloman.pezzottify.android.ui.theme.Spacing
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toArtist
import com.lelloman.pezzottify.android.ui.toProfile
import com.lelloman.pezzottify.android.ui.toSettings
import com.lelloman.pezzottify.android.ui.toTrack
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch


@Composable
fun HomeScreen(
    navController: NavController,
    onOpenProfileDrawer: () -> Unit = {},
) {
    val viewModel = hiltViewModel<HomeScreenViewModel>()
    HomeScreenContent(
        navController = navController,
        actions = viewModel,
        events = viewModel.events,
        state = viewModel.state.collectAsStateWithLifecycle().value,
        onOpenProfileDrawer = onOpenProfileDrawer,
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun HomeScreenContent(
    navController: NavController,
    events: Flow<HomeScreenEvents>,
    actions: HomeScreenActions,
    state: HomeScreenState,
    onOpenProfileDrawer: () -> Unit,
) {
    val coroutineScope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                HomeScreenEvents.NavigateToProfileScreen -> {
                    // Open the drawer instead of navigating to profile screen
                    onOpenProfileDrawer()
                }

                HomeScreenEvents.NavigateToSettingsScreen -> {
                    navController.toSettings()
                }

                is HomeScreenEvents.NavigateToAlbum -> navController.toAlbum(it.albumId)
                is HomeScreenEvents.NavigateToArtist -> navController.toArtist(it.artistId)
                is HomeScreenEvents.NavigateToTrack -> navController.toTrack(it.trackId)
            }
        }
    }

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        topBar = {
            TopAppBar(
                title = {
                    // Profile button with user initials
                    IconButton(onClick = {
                        coroutineScope.launch { actions.clickOnProfile() }
                    }) {
                        UserInitialsIcon(userName = state.userName)
                    }
                },
                actions = {
                    // Offline indicator (only visible when disconnected)
                    OfflineIndicator(
                        connectionState = state.connectionState,
                        modifier = Modifier
                            .padding(end = 4.dp)
                            .align(alignment = androidx.compose.ui.Alignment.CenterVertically),
                        size = 20.dp
                    )
                    // Settings icon
                    IconButton(onClick = {
                        coroutineScope.launch { actions.clickOnSettings() }
                    }) {
                        Icon(
                            imageVector = Icons.Default.Settings,
                            contentDescription = stringResource(R.string.settings),
                            tint = MaterialTheme.colorScheme.onSurface
                        )
                    }
                }
            )
        }
    ) { scaffoldPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(top = scaffoldPadding.calculateTopPadding())
        ) {
            // Back online banner (shows briefly when reconnecting)
            BackOnlineBanner(
                connectionState = state.connectionState,
                modifier = Modifier.fillMaxWidth()
            )

            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = Spacing.Medium),
            ) {
            state.recentlyViewedContent?.takeIf { it.isNotEmpty() }?.let { recentlyViewedItems ->
                Spacer(modifier = Modifier.height(Spacing.Medium))
                Text(
                    stringResource(R.string.recently_viewed_item_header),
                    style = MaterialTheme.typography.titleLarge
                )
                Spacer(modifier = Modifier.height(Spacing.Medium))

                val maxGroupSize = 2
                recentlyViewedItems.forEachGroup(maxGroupSize) { items ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
                    ) {
                        for (i in 0 until maxGroupSize) {
                            val item = items.getOrNull(i)
                            val itemState = item?.collectAsState(null)
                            itemState?.value?.let {
                                RecentlyViewedItem(
                                    modifier = Modifier
                                        .weight(1f),
                                    item = it,
                                    actions = actions
                                )
                            } ?: run {
                                Spacer(modifier = Modifier.weight(1f))
                            }
                        }
                    }
                    Spacer(modifier = Modifier.height(Spacing.Small))
                }
            }

                // Popular content section
                state.popularContent?.let { popularContent ->
                    if (popularContent.albums.isNotEmpty()) {
                        Spacer(modifier = Modifier.height(Spacing.Large))
                        Text(
                            stringResource(R.string.popular_albums_header),
                            style = MaterialTheme.typography.titleLarge
                        )
                        Spacer(modifier = Modifier.height(Spacing.Medium))
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .horizontalScroll(rememberScrollState()),
                            horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
                        ) {
                            popularContent.albums.forEach { album ->
                                PopularAlbumItem(
                                    album = album,
                                    onClick = { actions.clickOnPopularAlbum(album.id) }
                                )
                            }
                        }
                    }

                    if (popularContent.artists.isNotEmpty()) {
                        Spacer(modifier = Modifier.height(Spacing.Large))
                        Text(
                            stringResource(R.string.popular_artists_header),
                            style = MaterialTheme.typography.titleLarge
                        )
                        Spacer(modifier = Modifier.height(Spacing.Medium))
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .horizontalScroll(rememberScrollState()),
                            horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
                        ) {
                            popularContent.artists.forEach { artist ->
                                PopularArtistItem(
                                    artist = artist,
                                    onClick = { actions.clickOnPopularArtist(artist.id) }
                                )
                            }
                        }
                    }
                }

                // Show empty state message if no content
                val hasRecentlyViewed = state.recentlyViewedContent?.isNotEmpty() == true
                val hasPopularAlbums = state.popularContent?.albums?.isNotEmpty() == true
                val hasPopularArtists = state.popularContent?.artists?.isNotEmpty() == true
                if (!hasRecentlyViewed && !hasPopularAlbums && !hasPopularArtists) {
                    Spacer(modifier = Modifier.height(Spacing.ExtraLarge))
                    Text(
                        text = stringResource(R.string.home_empty_state),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.fillMaxWidth(),
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center
                    )
                }

                Spacer(modifier = Modifier.height(Spacing.Large))
            }
        }
    }
}

@Composable
private fun RecentlyViewedItem(
    modifier: Modifier,
    item: Content<ResolvedRecentlyViewedContent>,
    actions: HomeScreenActions
) {
    when (item) {
        is Content.Resolved -> {
            Card(
                modifier = modifier
                    .fillMaxWidth()
                    .clickable {
                        actions.clickOnRecentlyViewedItem(
                            item.itemId,
                            item.data.contentType
                        )
                    },
                shape = RoundedCornerShape(CornerRadius.Small),
                elevation = CardDefaults.cardElevation(
                    defaultElevation = Elevation.Small
                )
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(ComponentSize.ImageThumbMedium),
                    verticalAlignment = androidx.compose.ui.Alignment.CenterVertically
                ) {
                    NullablePezzottifyImage(
                        url = item.data.contentImageUrl,
                        shape = PezzottifyImageShape.FullSize,
                        modifier = Modifier
                            .fillMaxHeight()
                            .aspectRatio(1f, matchHeightConstraintsFirst = true)
                            .clip(
                                RoundedCornerShape(
                                    topStart = CornerRadius.Small,
                                    bottomStart = CornerRadius.Small,
                                    topEnd = 0.dp,
                                    bottomEnd = 0.dp
                                )
                            )
                    )
                    Column(
                        modifier = Modifier
                            .weight(1f)
                            .padding(Spacing.Small)
                    ) {
                        Text(
                            text = item.data.contentName,
                            style = MaterialTheme.typography.titleMedium,
                            maxLines = if (item.data.artists.isEmpty()) 2 else 1,
                            overflow = TextOverflow.Ellipsis,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        if (item.data.artists.isNotEmpty()) {
                            ScrollingArtistsRow(
                                artists = item.data.artists.map { ArtistInfo(it.id, it.name) },
                                textStyle = MaterialTheme.typography.bodySmall,
                            )
                        }
                        item.data.year?.let { year ->
                            Text(
                                text = year.toString(),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                            )
                        }
                    }
                }
            }
        }

        is Content.Loading -> {
            Box(
                modifier = modifier
                    .fillMaxWidth()
                    .height(ComponentSize.ImageThumbMedium),
                contentAlignment = androidx.compose.ui.Alignment.Center
            ) {
                CircularProgressIndicator()
            }
        }

        is Content.Error -> {
            Card(
                modifier = modifier
                    .fillMaxWidth(),
                shape = RoundedCornerShape(CornerRadius.Small),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.errorContainer
                )
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(ComponentSize.ImageThumbMedium)
                        .padding(Spacing.Small),
                    contentAlignment = androidx.compose.ui.Alignment.Center
                ) {
                    Text(
                        text = ":(",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onErrorContainer
                    )
                }
            }
        }
    }
}

@Composable
@Preview
private fun HomeScreenPreview() {
    val navController = rememberNavController()
    HomeScreen(navController = navController)
}

@Composable
private fun <T> List<T>.forEachGroup(maxGroupSize: Int, action: @Composable (List<T>) -> Unit) {

    val nGroups = size / maxGroupSize + (if (size % maxGroupSize > 0) 1 else 0)
    for (i in 0 until nGroups) {
        val start = i * maxGroupSize
        val end = minOf(start + maxGroupSize, size)
        action(subList(start, end))
    }
}

@Composable
private fun UserInitialsIcon(userName: String, modifier: Modifier = Modifier) {
    val initials = extractInitials(userName)

    Surface(
        modifier = modifier.size(40.dp),
        shape = androidx.compose.foundation.shape.CircleShape,
        color = MaterialTheme.colorScheme.primaryContainer
    ) {
        Box(
            contentAlignment = androidx.compose.ui.Alignment.Center,
            modifier = Modifier.fillMaxSize()
        ) {
            Text(
                text = initials,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onPrimaryContainer
            )
        }
    }
}

private fun extractInitials(userName: String): String {
    if (userName.isEmpty()) return "?"

    // If it's an email, take the part before @
    val nameToUse = if (userName.contains("@")) {
        userName.substringBefore("@")
    } else {
        userName
    }

    // Split by common delimiters (space, dot, underscore, dash)
    val parts = nameToUse.split(" ", ".", "_", "-").filter { it.isNotEmpty() }

    return when {
        parts.isEmpty() -> userName.take(1).uppercase()
        parts.size == 1 -> parts[0].take(2).uppercase()
        else -> (parts[0].take(1) + parts[1].take(1)).uppercase()
    }
}

@Composable
private fun PopularAlbumItem(
    album: PopularAlbumState,
    onClick: () -> Unit,
) {
    Card(
        modifier = Modifier
            .width(140.dp)
            .clickable(onClick = onClick),
        shape = RoundedCornerShape(CornerRadius.Small),
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Small)
    ) {
        Column {
            NullablePezzottifyImage(
                url = album.imageUrl,
                shape = PezzottifyImageShape.FullSize,
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(1f)
                    .clip(
                        RoundedCornerShape(
                            topStart = CornerRadius.Small,
                            topEnd = CornerRadius.Small,
                            bottomStart = 0.dp,
                            bottomEnd = 0.dp
                        )
                    )
            )
            Column(
                modifier = Modifier.padding(Spacing.Small)
            ) {
                Text(
                    text = album.name,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                if (album.artistNames.isNotEmpty()) {
                    Text(
                        text = album.artistNames.joinToString(", "),
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun PopularArtistItem(
    artist: PopularArtistState,
    onClick: () -> Unit,
) {
    Card(
        modifier = Modifier
            .width(140.dp)
            .clickable(onClick = onClick),
        shape = RoundedCornerShape(CornerRadius.Small),
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Small)
    ) {
        Column {
            NullablePezzottifyImage(
                url = artist.imageUrl,
                shape = PezzottifyImageShape.FullSize,
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(1f)
                    .clip(
                        RoundedCornerShape(
                            topStart = CornerRadius.Small,
                            topEnd = CornerRadius.Small,
                            bottomStart = 0.dp,
                            bottomEnd = 0.dp
                        )
                    )
            )
            Column(
                modifier = Modifier.padding(Spacing.Small)
            ) {
                Text(
                    text = artist.name,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
    }
}