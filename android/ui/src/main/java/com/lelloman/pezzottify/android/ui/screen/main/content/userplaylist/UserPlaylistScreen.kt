package com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.tween
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
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.bottomsheet.PlaylistPickerBottomSheet
import com.lelloman.pezzottify.android.ui.component.bottomsheet.TrackActionsBottomSheet
import com.lelloman.pezzottify.android.ui.component.dialog.CreatePlaylistDialog
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toTrack
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
        navController = navController,
        actions = viewModel
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun UserPlaylistScreenContent(
    state: UserPlaylistScreenState,
    contentResolver: ContentResolver,
    navController: NavController,
    actions: UserPlaylistScreenActions
) {
    val context = LocalContext.current
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    // Bottom sheet states
    val trackSheetState = rememberModalBottomSheetState()
    val playlistPickerSheetState = rememberModalBottomSheetState()

    // Selected item for bottom sheets
    var selectedTrack by remember { mutableStateOf<Track?>(null) }
    var selectedTrackIsLiked by remember { mutableStateOf(false) }
    var showPlaylistPicker by remember { mutableStateOf(false) }
    var showCreatePlaylistDialog by remember { mutableStateOf(false) }

    // Collect like state for the selected track
    selectedTrack?.let { track ->
        val likeState by actions.getTrackLikeState(track.id).collectAsState(initial = false)
        selectedTrackIsLiked = likeState
    }

    // Track pending add to playlist
    var pendingAddToPlaylistTrackId by remember { mutableStateOf<String?>(null) }

    val showSnackbar: (String) -> Unit = { message ->
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
                    onShowSnackbar = showSnackbar,
                    onTrackMoreClick = { track -> selectedTrack = track },
                )
            }
        }
    }

    // Track actions bottom sheet
    selectedTrack?.let { track ->
        TrackActionsBottomSheet(
            track = track,
            sheetState = trackSheetState,
            onDismiss = { selectedTrack = null },
            onPlay = {
                actions.clickOnTrack(track.id)
            },
            onPlaySingle = {
                actions.playTrackDirectly(track.id)
            },
            onAddToQueue = {
                actions.addTrackToQueue(track.id)
                showSnackbar(context.getString(R.string.added_to_queue))
            },
            onAddToPlaylist = {
                pendingAddToPlaylistTrackId = track.id
                showPlaylistPicker = true
            },
            onRemoveFromPlaylist = {
                actions.removeTrackFromPlaylist(track.id)
                showSnackbar(context.getString(R.string.removed_from_playlist))
            },
            onViewTrack = {
                navController.toTrack(track.id)
            },
            onViewAlbum = {
                navController.toAlbum(track.albumId)
            },
            isLiked = selectedTrackIsLiked,
            onToggleLike = {
                actions.toggleTrackLike(track.id, selectedTrackIsLiked)
            },
        )
    }

    // Playlist picker bottom sheet
    if (showPlaylistPicker) {
        PlaylistPickerBottomSheet(
            playlists = state.userPlaylists,
            sheetState = playlistPickerSheetState,
            onDismiss = {
                showPlaylistPicker = false
                pendingAddToPlaylistTrackId = null
            },
            onPlaylistSelected = { playlistId ->
                pendingAddToPlaylistTrackId?.let { trackId ->
                    actions.addTrackToPlaylist(trackId, playlistId)
                    showSnackbar(context.getString(R.string.added_to_playlist))
                }
                showPlaylistPicker = false
                pendingAddToPlaylistTrackId = null
            },
            onCreateNewPlaylist = {
                showCreatePlaylistDialog = true
            },
        )
    }

    // Create playlist dialog
    if (showCreatePlaylistDialog) {
        CreatePlaylistDialog(
            onDismiss = { showCreatePlaylistDialog = false },
            onCreate = { name ->
                actions.createPlaylist(name)
                showSnackbar(context.getString(R.string.playlist_created))
            },
        )
    }
}

@Composable
private fun ErrorScreen() {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(
            text = stringResource(R.string.could_not_load_playlist),
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
    onShowSnackbar: (String) -> Unit,
    onTrackMoreClick: (Track) -> Unit,
) {
    val playlistAddedToQueueMessage = stringResource(R.string.playlist_added_to_queue)
    Column(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.statusBars)
    ) {
        // Header
        PlaylistHeader(
            playlistName = state.playlistName,
            trackCount = state.tracks?.size ?: 0,
            onPlayClick = { actions.clickOnPlayPlaylist() },
            onAddToQueueClick = {
                actions.addPlaylistToQueue()
                onShowSnackbar(playlistAddedToQueueMessage)
            },
        )

        // Track list
        state.tracks?.let { trackFlows ->
            TrackList(
                trackFlows = trackFlows,
                currentPlayingTrackId = state.currentPlayingTrackId,
                onTrackClick = onTrackMoreClick,
            )
        }
    }
}

@Composable
private fun PlaylistHeader(
    playlistName: String,
    trackCount: Int,
    onPlayClick: () -> Unit,
    onAddToQueueClick: () -> Unit,
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
                    text = pluralStringResource(R.plurals.tracks_count, trackCount, trackCount),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }

            // Add to queue button
            IconButton(
                onClick = onAddToQueueClick,
                modifier = Modifier.size(48.dp)
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(
                        modifier = Modifier.size(48.dp),
                        painter = painterResource(R.drawable.baseline_circle_24),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.surfaceVariant,
                    )
                    Icon(
                        modifier = Modifier.size(24.dp),
                        painter = painterResource(R.drawable.baseline_playlist_add_24),
                        contentDescription = stringResource(R.string.add_to_queue),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
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
                        painter = painterResource(R.drawable.baseline_play_arrow_24),
                        contentDescription = stringResource(R.string.play),
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
    onTrackClick: (Track) -> Unit,
) {
    LazyColumn(modifier = Modifier.fillMaxSize()) {
        items(trackFlows) { trackFlow ->
            when (val track = trackFlow.collectAsState(initial = null).value) {
                is Content.Resolved -> TrackItem(
                    track = track.data,
                    isPlaying = track.data.id == currentPlayingTrackId,
                    onClick = { onTrackClick(track.data) },
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
    val textColor = when {
        track.isUnavailable -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.4f)
        isPlaying -> MaterialTheme.colorScheme.primary
        else -> MaterialTheme.colorScheme.onSurface
    }
    val secondaryTextColor = if (track.isUnavailable) {
        MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant
    }

    // Pulsing animation for fetching state
    val fetchingAlpha by animateFloatAsState(
        targetValue = if (track.isFetching) 0.4f else 1f,
        animationSpec = if (track.isFetching) {
            infiniteRepeatable(
                animation = tween(durationMillis = 750, easing = LinearEasing),
                repeatMode = RepeatMode.Reverse
            )
        } else {
            tween(durationMillis = 0)
        },
        label = "fetchingAlpha"
    )

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(backgroundColor)
            .alpha(
                when {
                    track.isFetching -> fetchingAlpha
                    track.isUnavailable -> 0.4f
                    else -> 1f
                }
            )
            .clickable(enabled = track.isPlayable, onClick = onClick)
            .padding(start = 16.dp, top = 8.dp, bottom = 8.dp, end = 16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // Warning icon for fetch error
        if (track.isFetchError) {
            Text(
                text = "⚠",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(end = 8.dp)
            )
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.name,
                style = MaterialTheme.typography.bodyLarge,
                color = textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                text = track.artists.joinToString(", ") { it.name },
                style = MaterialTheme.typography.bodyMedium,
                color = secondaryTextColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        DurationText(
            durationSeconds = track.durationSeconds,
            modifier = Modifier.padding(start = 8.dp)
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
        PezzottifyLoader(size = LoaderSize.Small)
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
            text = stringResource(R.string.error_loading_track),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error
        )
    }
}
