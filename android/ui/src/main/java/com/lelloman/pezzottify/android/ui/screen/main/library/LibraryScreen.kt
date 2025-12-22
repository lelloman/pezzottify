package com.lelloman.pezzottify.android.ui.screen.main.library

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.AlbumGridItem
import com.lelloman.pezzottify.android.ui.component.ArtistGridItem
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.PlaylistGridItem
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.component.dialog.CreatePlaylistDialog
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toArtist
import com.lelloman.pezzottify.android.ui.toTrack
import com.lelloman.pezzottify.android.ui.toUserPlaylist

private enum class LibraryTab {
    Albums,
    Artists,
    Tracks,
    Playlists,
}

@Composable
fun LibraryScreen(navController: NavController) {
    val viewModel = hiltViewModel<LibraryScreenViewModel>()
    LibraryScreenContent(
        state = viewModel.state.collectAsState().value,
        actions = viewModel,
        contentResolver = viewModel.contentResolver,
        navController = navController,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LibraryScreenContent(
    state: LibraryScreenState,
    actions: LibraryScreenActions,
    contentResolver: ContentResolver,
    navController: NavController,
) {
    var selectedTab by rememberSaveable { mutableStateOf(LibraryTab.Albums) }
    var showCreatePlaylistDialog by remember { mutableStateOf(false) }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.statusBars)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Header with segmented button
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp)
            ) {
                Text(
                    text = stringResource(R.string.your_library),
                    style = MaterialTheme.typography.headlineMedium,
                    modifier = Modifier.padding(bottom = 12.dp)
                )

                SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                    LibraryTab.entries.forEachIndexed { index, tab ->
                        SegmentedButton(
                            shape = SegmentedButtonDefaults.itemShape(
                                index = index,
                                count = LibraryTab.entries.size
                            ),
                            onClick = { selectedTab = tab },
                            selected = selectedTab == tab
                        ) {
                            AutoShrinkText(text = tab.name)
                        }
                    }
                }
            }

            // Content area
            Box(modifier = Modifier.fillMaxSize()) {
                when {
                    state.isLoading -> LoadingScreen()
                    else -> LibraryLoadedScreen(
                        state = state,
                        selectedTab = selectedTab,
                        contentResolver = contentResolver,
                        navController = navController,
                    )
                }
            }
        }

        // FAB for creating playlist (only shown on Playlists tab)
        if (selectedTab == LibraryTab.Playlists) {
            FloatingActionButton(
                onClick = { showCreatePlaylistDialog = true },
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(16.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.Add,
                    contentDescription = stringResource(R.string.create_playlist),
                )
            }
        }
    }

    if (showCreatePlaylistDialog) {
        CreatePlaylistDialog(
            onDismiss = { showCreatePlaylistDialog = false },
            onCreate = { name ->
                actions.createPlaylist(name)
            },
        )
    }
}

@Composable
private fun EmptyLibraryScreen(
    title: String,
    subtitle: String,
) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleLarge,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
    }
}

@Composable
private fun LibraryLoadedScreen(
    state: LibraryScreenState,
    selectedTab: LibraryTab,
    contentResolver: ContentResolver,
    navController: NavController,
) {
    when (selectedTab) {
        LibraryTab.Albums -> {
            if (state.likedAlbumIds.isEmpty()) {
                EmptyLibraryScreen(
                    title = stringResource(R.string.no_liked_albums_yet),
                    subtitle = stringResource(R.string.albums_you_like_will_appear_here)
                )
            } else {
                LazyColumn(modifier = Modifier.fillMaxSize()) {
                    item {
                        AlbumGrid(
                            albumIds = state.likedAlbumIds,
                            contentResolver = contentResolver,
                            navController = navController,
                        )
                    }
                }
            }
        }
        LibraryTab.Artists -> {
            if (state.likedArtistIds.isEmpty()) {
                EmptyLibraryScreen(
                    title = stringResource(R.string.no_liked_artists_yet),
                    subtitle = stringResource(R.string.artists_you_like_will_appear_here)
                )
            } else {
                LazyColumn(modifier = Modifier.fillMaxSize()) {
                    item {
                        ArtistGrid(
                            artistIds = state.likedArtistIds,
                            contentResolver = contentResolver,
                            navController = navController,
                        )
                    }
                }
            }
        }
        LibraryTab.Tracks -> {
            if (state.likedTrackIds.isEmpty()) {
                EmptyLibraryScreen(
                    title = stringResource(R.string.no_liked_tracks_yet),
                    subtitle = stringResource(R.string.tracks_you_like_will_appear_here)
                )
            } else {
                TrackList(
                    trackIds = state.likedTrackIds,
                    contentResolver = contentResolver,
                    onTrackClick = { trackId -> navController.toTrack(trackId) },
                )
            }
        }
        LibraryTab.Playlists -> {
            if (state.playlists.isEmpty()) {
                EmptyLibraryScreen(
                    title = stringResource(R.string.no_playlists_yet),
                    subtitle = stringResource(R.string.your_playlists_will_appear_here)
                )
            } else {
                LazyColumn(modifier = Modifier.fillMaxSize()) {
                    item {
                        PlaylistGrid(
                            playlists = state.playlists,
                            navController = navController,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun AlbumGrid(
    albumIds: List<String>,
    contentResolver: ContentResolver,
    navController: NavController,
) {
    val maxGroupSize = 2
    albumIds.forEachGroup(maxGroupSize) { ids ->
        Row(modifier = Modifier.fillMaxWidth()) {
            for (i in 0 until maxGroupSize) {
                val albumId = ids.getOrNull(i)
                if (albumId != null) {
                    val albumFlow = contentResolver.resolveAlbum(albumId)
                    val albumState = albumFlow.collectAsState(Content.Loading(albumId))
                    when (val album = albumState.value) {
                        is Content.Resolved -> {
                            AlbumGridItem(
                                modifier = Modifier.weight(1f),
                                albumName = album.data.name,
                                albumDate = album.data.date,
                                albumCoverUrl = album.data.imageUrl,
                                onClick = { navController.toAlbum(albumId) }
                            )
                        }
                        is Content.Loading, is Content.Error -> {
                            Spacer(modifier = Modifier.weight(1f))
                        }
                    }
                } else {
                    Spacer(modifier = Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun ArtistGrid(
    artistIds: List<String>,
    contentResolver: ContentResolver,
    navController: NavController,
) {
    val maxGroupSize = 2
    artistIds.forEachGroup(maxGroupSize) { ids ->
        Row(modifier = Modifier.fillMaxWidth()) {
            for (i in 0 until maxGroupSize) {
                val artistId = ids.getOrNull(i)
                if (artistId != null) {
                    val artistFlow = contentResolver.resolveArtist(artistId)
                    val artistState = artistFlow.collectAsState(Content.Loading(artistId))
                    when (val artist = artistState.value) {
                        is Content.Resolved -> {
                            ArtistGridItem(
                                modifier = Modifier.weight(1f),
                                artistName = artist.data.name,
                                artistImageUrl = artist.data.imageUrl,
                                onClick = { navController.toArtist(artistId) }
                            )
                        }
                        is Content.Loading, is Content.Error -> {
                            Spacer(modifier = Modifier.weight(1f))
                        }
                    }
                } else {
                    Spacer(modifier = Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun PlaylistGrid(
    playlists: List<UiUserPlaylist>,
    navController: NavController,
) {
    val maxGroupSize = 2
    playlists.forEachGroup(maxGroupSize) { items ->
        Row(modifier = Modifier.fillMaxWidth()) {
            for (i in 0 until maxGroupSize) {
                val playlist = items.getOrNull(i)
                if (playlist != null) {
                    PlaylistGridItem(
                        modifier = Modifier.weight(1f),
                        playlistName = playlist.name,
                        trackCount = playlist.trackCount,
                        onClick = { navController.toUserPlaylist(playlist.id) },
                    )
                } else {
                    Spacer(modifier = Modifier.weight(1f))
                }
            }
        }
    }
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
private fun TrackList(
    trackIds: List<String>,
    contentResolver: ContentResolver,
    onTrackClick: (String) -> Unit,
) {
    LazyColumn(modifier = Modifier.fillMaxSize()) {
        items(trackIds) { trackId ->
            val trackFlow = contentResolver.resolveTrack(trackId)
            val trackState = trackFlow.collectAsState(Content.Loading(trackId))
            when (val track = trackState.value) {
                is Content.Resolved -> TrackListItem(
                    track = track.data,
                    onClick = { onTrackClick(trackId) },
                )
                is Content.Loading -> LoadingTrackListItem()
                is Content.Error -> ErrorTrackListItem()
            }
        }
    }
}

@Composable
private fun TrackListItem(
    track: Track,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.name,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            ScrollingArtistsRow(artists = track.artists)
        }
        DurationText(
            durationSeconds = track.durationSeconds,
            modifier = Modifier.padding(start = 16.dp)
        )
    }
}

@Composable
private fun LoadingTrackListItem() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        PezzottifyLoader(size = LoaderSize.Small)
    }
}

@Composable
private fun ErrorTrackListItem() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = stringResource(R.string.error_loading_track),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error
        )
    }
}

@Composable
private fun AutoShrinkText(
    text: String,
    modifier: Modifier = Modifier,
) {
    var fontSize by remember(text) { mutableStateOf(14.sp) }
    val minFontSize = 10.sp

    Text(
        text = text,
        modifier = modifier,
        fontSize = fontSize,
        maxLines = 1,
        softWrap = false,
        onTextLayout = { result ->
            if (result.didOverflowWidth && fontSize > minFontSize) {
                fontSize = (fontSize.value - 1f).sp
            }
        },
    )
}
