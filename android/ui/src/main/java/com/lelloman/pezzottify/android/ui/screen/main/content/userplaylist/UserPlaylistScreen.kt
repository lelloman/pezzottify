package com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch

@Composable
fun UserPlaylistScreen(playlistId: String, navController: NavController) {
    val viewModel = hiltViewModel<UserPlaylistScreenViewModel, UserPlaylistScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(playlistId = playlistId) }
    )
    UserPlaylistScreenContent(
        state = viewModel.state.collectAsState().value,
        contentResolver = viewModel.contentResolver,
        actions = viewModel
    )
}

@Composable
private fun UserPlaylistScreenContent(
    state: UserPlaylistScreenState,
    contentResolver: ContentResolver,
    actions: UserPlaylistScreenActions
) {
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    val showAddedToQueueSnackbar: (String) -> Unit = { message ->
        scope.launch {
            snackbarHostState.showSnackbar(message)
        }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) },
        contentWindowInsets = WindowInsets(0.dp)
    ) { contentPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(contentPadding)
        ) {
            when {
                state.isLoading -> LoadingScreen()
                state.isError -> ErrorScreen()
                else -> UserPlaylistLoadedScreen(
                    state = state,
                    contentResolver = contentResolver,
                    actions = actions,
                    onAddedToQueue = showAddedToQueueSnackbar,
                )
            }
        }
    }
}

@Composable
private fun ErrorScreen() {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(
            text = "Could not load playlist",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.error,
        )
    }
}

@Composable
private fun UserPlaylistLoadedScreen(
    state: UserPlaylistScreenState,
    contentResolver: ContentResolver,
    actions: UserPlaylistScreenActions,
    onAddedToQueue: (String) -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.statusBars)
    ) {
        // Header
        PlaylistHeader(
            playlistName = state.playlistName,
            trackCount = state.tracks?.size ?: 0,
            isAddToQueueMode = state.isAddToQueueMode,
            onPlayClick = {
                actions.clickOnPlayPlaylist()
                if (state.isAddToQueueMode) {
                    onAddedToQueue("Playlist added to queue")
                }
            },
        )

        // Track list
        state.tracks?.let { trackFlows ->
            TrackList(
                trackFlows = trackFlows,
                currentPlayingTrackId = state.currentPlayingTrackId,
                isAddToQueueMode = state.isAddToQueueMode,
                onTrackClick = { trackId ->
                    actions.clickOnTrack(trackId)
                    if (state.isAddToQueueMode) {
                        onAddedToQueue("Added to queue")
                    }
                },
            )
        }
    }
}

@Composable
private fun PlaylistHeader(
    playlistName: String,
    trackCount: Int,
    isAddToQueueMode: Boolean,
    onPlayClick: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.fillMaxWidth()
        ) {
            // Playlist icon
            Box(
                modifier = Modifier
                    .size(80.dp)
                    .background(
                        color = MaterialTheme.colorScheme.primaryContainer,
                        shape = MaterialTheme.shapes.medium
                    ),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    painter = painterResource(R.drawable.baseline_queue_music_24),
                    contentDescription = null,
                    modifier = Modifier.size(40.dp),
                    tint = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            }

            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(start = 16.dp)
            ) {
                Text(
                    text = playlistName,
                    style = MaterialTheme.typography.headlineSmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = "$trackCount tracks",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }

            // Play button
            IconButton(
                onClick = onPlayClick,
                modifier = Modifier.size(56.dp)
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(
                        modifier = Modifier.size(56.dp),
                        painter = painterResource(R.drawable.baseline_circle_24),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                    )
                    Icon(
                        modifier = Modifier.size(28.dp),
                        painter = painterResource(
                            if (isAddToQueueMode) R.drawable.baseline_playlist_add_24
                            else R.drawable.baseline_play_arrow_24
                        ),
                        contentDescription = if (isAddToQueueMode) "Add to queue" else "Play",
                        tint = MaterialTheme.colorScheme.onPrimary,
                    )
                }
            }
        }
    }
}

@Composable
private fun TrackList(
    trackFlows: List<Flow<Content<Track>>>,
    currentPlayingTrackId: String?,
    isAddToQueueMode: Boolean,
    onTrackClick: (String) -> Unit,
) {
    LazyColumn(modifier = Modifier.fillMaxSize()) {
        items(trackFlows) { trackFlow ->
            when (val track = trackFlow.collectAsState(initial = null).value) {
                is Content.Resolved -> TrackItem(
                    track = track.data,
                    isPlaying = track.data.id == currentPlayingTrackId,
                    onClick = { onTrackClick(track.data.id) },
                )
                null, is Content.Loading -> LoadingTrackItem()
                is Content.Error -> ErrorTrackItem()
            }
        }
    }
}

@Composable
private fun TrackItem(
    track: Track,
    isPlaying: Boolean,
    onClick: () -> Unit,
) {
    val backgroundColor = if (isPlaying) {
        MaterialTheme.colorScheme.primaryContainer
    } else {
        Color.Transparent
    }
    val textColor = if (isPlaying) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurface
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(backgroundColor)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.name,
                style = MaterialTheme.typography.bodyLarge,
                color = textColor,
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
private fun LoadingTrackItem() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        CircularProgressIndicator(modifier = Modifier.size(24.dp))
    }
}

@Composable
private fun ErrorTrackItem() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = "Error loading track",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error
        )
    }
}
