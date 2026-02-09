package com.lelloman.pezzottify.android.ui.screen.queue

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.tween
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
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
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
import androidx.compose.ui.draw.alpha
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
    var selectedTrackIndex by remember { mutableStateOf<Int?>(null) }
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

    fun removeTrackWithUndo(index: Int, trackItem: QueueTrackItem) {
        val originalSize = state.tracks.size
        actions.removeTrack(index)
        scope.launch {
            val result = snackbarHostState.showSnackbar(
                message = context.getString(R.string.removed_from_queue),
                actionLabel = context.getString(R.string.undo),
            )
            if (result == SnackbarResult.ActionPerformed) {
                actions.addTrackToQueue(trackItem.trackId)
                val fromIndex = originalSize - 1
                if (fromIndex >= 0 && index <= fromIndex) {
                    actions.moveTrack(fromIndex, index)
                }
            }
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
                onTrackClick = { index, trackItem ->
                    selectedTrackIndex = index
                    selectedTrackItem = trackItem
                },
                onRemoveTrack = { index, trackItem -> removeTrackWithUndo(index, trackItem) },
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
            onDismiss = {
                selectedTrackItem = null
                selectedTrackIndex = null
            },
            onPlay = {
                selectedTrackIndex?.let { index ->
                    actions.clickOnTrack(index)
                }
            },
            onPlaySingle = {
                actions.playTrackDirectly(trackItem.trackId)
            },
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
    onTrackClick: (Int, QueueTrackItem) -> Unit,
    onRemoveTrack: (Int, QueueTrackItem) -> Unit,
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
                onClick = { onTrackClick(index, trackItem) },
                onRemoveTrack = { onRemoveTrack(index, trackItem) },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SwipeableQueueItem(
    index: Int,
    trackItem: QueueTrackItem,
    isCurrentTrack: Boolean,
    onClick: () -> Unit,
    onRemoveTrack: () -> Unit,
) {
    val dismissState = rememberSwipeToDismissBoxState(
        confirmValueChange = { dismissValue ->
            if (dismissValue == SwipeToDismissBoxValue.EndToStart) {
                onRemoveTrack()
                true
            } else {
                false
            }
        }
    )

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
                onClick = onClick,
            )
        }
    )
}

@Composable
private fun QueueTrackItemRow(
    index: Int,
    trackItem: QueueTrackItem,
    isCurrentTrack: Boolean,
    onClick: () -> Unit,
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

    val textColor = when {
        trackItem.isUnavailable -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.4f)
        isCurrentTrack -> MaterialTheme.colorScheme.primary
        else -> MaterialTheme.colorScheme.onSurface
    }

    val secondaryTextColor = if (trackItem.isUnavailable) {
        MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant
    }

    // Pulsing animation for fetching state
    val fetchingAlpha by animateFloatAsState(
        targetValue = if (trackItem.isFetching) 0.4f else 1f,
        animationSpec = if (trackItem.isFetching) {
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
            .shadow(elevation)
            .background(backgroundColor)
            .alpha(if (trackItem.isFetching) fetchingAlpha else 1f)
            .clickable(enabled = trackItem.isPlayable, onClick = onClick)
            .padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Track number or warning icon
        if (trackItem.isFetchError) {
            Text(
                text = "⚠",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.width(32.dp),
            )
        } else {
            Text(
                text = "${index + 1}",
                style = MaterialTheme.typography.bodyMedium,
                color = secondaryTextColor,
                modifier = Modifier.width(32.dp),
            )
        }

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
            Text(
                text = trackItem.artists.joinToString(", ") { it.name },
                style = MaterialTheme.typography.bodyMedium,
                color = secondaryTextColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }

        // Duration
        DurationText(
            durationSeconds = trackItem.durationSeconds,
            modifier = Modifier.padding(start = 8.dp)
        )
    }
}
