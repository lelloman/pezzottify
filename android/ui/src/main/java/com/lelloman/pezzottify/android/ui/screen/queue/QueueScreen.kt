package com.lelloman.pezzottify.android.ui.screen.queue

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.component.bottomsheet.PlaylistPickerBottomSheet
import com.lelloman.pezzottify.android.ui.component.bottomsheet.TrackActionsBottomSheet
import com.lelloman.pezzottify.android.ui.component.dialog.CreatePlaylistDialog
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toTrack

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QueueScreen(navController: NavController) {
    val viewModel = hiltViewModel<QueueScreenViewModel>()
    val state by viewModel.state.collectAsState()
    QueueScreenContent(state = state, actions = viewModel, navController = navController)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun QueueScreenContent(
    state: QueueScreenState,
    actions: QueueScreenActions,
    navController: NavController,
) {
    val context = LocalContext.current
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    // Bottom sheet states
    val trackSheetState = rememberModalBottomSheetState()
    val playlistPickerSheetState = rememberModalBottomSheetState()

    // Selected item for bottom sheets
    var selectedTrackItem by remember { mutableStateOf<QueueTrackItem?>(null) }
    var selectedTrackIsLiked by remember { mutableStateOf(false) }
    var showPlaylistPicker by remember { mutableStateOf(false) }
    var showCreatePlaylistDialog by remember { mutableStateOf(false) }

    // Collect like state for the selected track
    selectedTrackItem?.let { trackItem ->
        val likeState by actions.getTrackLikeState(trackItem.trackId).collectAsState(initial = false)
        selectedTrackIsLiked = likeState
    }

    // Collect user playlists for the picker
    val userPlaylists by actions.getUserPlaylists().collectAsState(initial = emptyList())

    // Track pending add to playlist
    var pendingAddToPlaylistTrackId by remember { mutableStateOf<String?>(null) }

    val showSnackbar: (String) -> Unit = { message ->
        scope.launch {
            snackbarHostState.showSnackbar(message)
        }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) },
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(
                            text = stringResource(R.string.queue_title),
                            style = MaterialTheme.typography.titleLarge,
                        )
                        if (state.contextName.isNotEmpty()) {
                            Text(
                                text = when (state.contextType) {
                                    QueueContextType.Album -> stringResource(R.string.playing_from_album)
                                    QueueContextType.UserPlaylist -> stringResource(R.string.playing_from_playlist)
                                    QueueContextType.UserMix -> stringResource(R.string.your_mix)
                                    QueueContextType.Unknown -> ""
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                },
                navigationIcon = {
                    IconButton(onClick = { navController.popBackStack() }) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.KeyboardArrowLeft,
                            contentDescription = stringResource(R.string.back),
                            modifier = Modifier.size(32.dp)
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                )
            )
        }
    ) { innerPadding ->
        when {
            state.isLoading -> LoadingScreen()
            state.isError -> ErrorContent()
            else -> QueueList(
                tracks = state.tracks,
                currentTrackIndex = state.currentTrackIndex,
                actions = actions,
                onTrackMoreClick = { trackItem -> selectedTrackItem = trackItem },
                modifier = Modifier.padding(innerPadding),
            )
        }
    }

    // Track actions bottom sheet
    selectedTrackItem?.let { trackItem ->
        // Convert QueueTrackItem to Track for the bottom sheet
        val track = Track(
            id = trackItem.trackId,
            name = trackItem.trackName,
            albumId = trackItem.albumId,
            artists = trackItem.artists,
            durationSeconds = trackItem.durationSeconds,
        )
        TrackActionsBottomSheet(
            track = track,
            sheetState = trackSheetState,
            onDismiss = { selectedTrackItem = null },
            onAddToQueue = {
                actions.addTrackToQueue(trackItem.trackId)
                showSnackbar(context.getString(R.string.added_to_queue))
            },
            onAddToPlaylist = {
                pendingAddToPlaylistTrackId = trackItem.trackId
                showPlaylistPicker = true
            },
            onViewTrack = {
                navController.toTrack(trackItem.trackId)
            },
            onViewAlbum = {
                navController.toAlbum(trackItem.albumId)
            },
            isLiked = selectedTrackIsLiked,
            onToggleLike = {
                actions.toggleTrackLike(trackItem.trackId, selectedTrackIsLiked)
            },
        )
    }

    // Playlist picker bottom sheet
    if (showPlaylistPicker) {
        PlaylistPickerBottomSheet(
            playlists = userPlaylists,
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
private fun ErrorContent() {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = stringResource(R.string.no_playback_queue),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun QueueList(
    tracks: List<QueueTrackItem>,
    currentTrackIndex: Int?,
    actions: QueueScreenActions,
    onTrackMoreClick: (QueueTrackItem) -> Unit,
    modifier: Modifier = Modifier,
) {
    val listState = rememberLazyListState()

    // Auto-scroll to current track when first loaded
    LaunchedEffect(currentTrackIndex) {
        currentTrackIndex?.let { index ->
            if (index in tracks.indices) {
                listState.animateScrollToItem(index)
            }
        }
    }

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
    ) {
        itemsIndexed(
            items = tracks,
            key = { index, item -> "$index-${item.trackId}" }
        ) { index, trackItem ->
            SwipeableQueueItem(
                index = index,
                trackItem = trackItem,
                isCurrentTrack = index == currentTrackIndex,
                onPlayTrack = { actions.clickOnTrack(index) },
                onRemoveTrack = { actions.removeTrack(trackItem.trackId) },
                onMoreClick = { onTrackMoreClick(trackItem) },
            )
        }
    }
}

private const val UNDO_TIMEOUT_MS = 5000L

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SwipeableQueueItem(
    index: Int,
    trackItem: QueueTrackItem,
    isCurrentTrack: Boolean,
    onPlayTrack: () -> Unit,
    onRemoveTrack: () -> Unit,
    onMoreClick: () -> Unit,
) {
    var isPendingDeletion by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    val dismissState = rememberSwipeToDismissBoxState(
        confirmValueChange = { dismissValue ->
            if (dismissValue == SwipeToDismissBoxValue.EndToStart) {
                isPendingDeletion = true
                // Start the countdown to actually delete
                scope.launch {
                    delay(UNDO_TIMEOUT_MS)
                    if (isPendingDeletion) {
                        onRemoveTrack()
                    }
                }
                false // Don't dismiss, we'll show the undo UI instead
            } else {
                false
            }
        }
    )

    // Reset dismiss state when undo UI is shown
    LaunchedEffect(isPendingDeletion) {
        if (isPendingDeletion) {
            dismissState.reset()
        }
    }

    if (isPendingDeletion) {
        // Show undo placeholder
        PendingDeletionItem(
            trackName = trackItem.trackName,
            onUndo = { isPendingDeletion = false }
        )
    } else {
        SwipeToDismissBox(
            state = dismissState,
            enableDismissFromStartToEnd = false,
            backgroundContent = {
                val color by animateColorAsState(
                    when (dismissState.targetValue) {
                        SwipeToDismissBoxValue.EndToStart -> MaterialTheme.colorScheme.errorContainer
                        else -> Color.Transparent
                    },
                    label = "dismissBackground"
                )
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(color)
                        .padding(horizontal = 20.dp),
                    contentAlignment = Alignment.CenterEnd,
                ) {
                    if (dismissState.targetValue == SwipeToDismissBoxValue.EndToStart) {
                        Icon(
                            imageVector = Icons.Default.Delete,
                            contentDescription = stringResource(R.string.remove_from_queue),
                            tint = MaterialTheme.colorScheme.onErrorContainer,
                        )
                    }
                }
            },
            content = {
                QueueTrackItemRow(
                    index = index,
                    trackItem = trackItem,
                    isCurrentTrack = isCurrentTrack,
                    onClick = onPlayTrack,
                    onMoreClick = onMoreClick,
                )
            }
        )
    }
}

@Composable
private fun PendingDeletionItem(
    trackName: String,
    onUndo: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.errorContainer)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = stringResource(R.string.removed_from_queue),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onErrorContainer,
            modifier = Modifier.weight(1f),
        )
        TextButton(onClick = onUndo) {
            Text(
                text = stringResource(R.string.undo),
                color = MaterialTheme.colorScheme.onErrorContainer,
                style = MaterialTheme.typography.labelLarge,
            )
        }
    }
}

@Composable
private fun QueueTrackItemRow(
    index: Int,
    trackItem: QueueTrackItem,
    isCurrentTrack: Boolean,
    onClick: () -> Unit,
    onMoreClick: () -> Unit,
) {
    val elevation by animateDpAsState(
        if (isCurrentTrack) 4.dp else 0.dp,
        label = "elevation"
    )

    val backgroundColor = if (isCurrentTrack) {
        MaterialTheme.colorScheme.primaryContainer
    } else {
        MaterialTheme.colorScheme.surface
    }

    val textColor = if (isCurrentTrack) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurface
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .shadow(elevation)
            .background(backgroundColor)
            .clickable(onClick = onClick)
            .padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Track number
        Text(
            text = "${index + 1}",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(32.dp),
        )

        // Track info
        Column(
            modifier = Modifier.weight(1f)
        ) {
            Text(
                text = trackItem.trackName,
                style = MaterialTheme.typography.bodyLarge,
                color = textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            ScrollingArtistsRow(
                artists = trackItem.artists
            )
        }

        // Duration
        DurationText(
            durationSeconds = trackItem.durationSeconds,
            modifier = Modifier.padding(start = 8.dp)
        )

        // More options button
        IconButton(
            onClick = onMoreClick,
            modifier = Modifier.size(40.dp)
        ) {
            Icon(
                painter = painterResource(R.drawable.baseline_more_vert_24),
                contentDescription = stringResource(R.string.more_options),
                tint = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
