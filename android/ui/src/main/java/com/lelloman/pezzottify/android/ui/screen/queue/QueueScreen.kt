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
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.Track

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
    Scaffold(
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
                modifier = Modifier.padding(innerPadding),
            )
        }
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
            val trackContent by trackItem.trackFlow.collectAsState(initial = Content.Loading(trackItem.trackId))

            SwipeableQueueItem(
                index = index,
                trackId = trackItem.trackId,
                trackContent = trackContent,
                isCurrentTrack = index == currentTrackIndex,
                onPlayTrack = { actions.clickOnTrack(index) },
                onRemoveTrack = { actions.removeTrack(trackItem.trackId) },
            )
        }
    }
}

private const val UNDO_TIMEOUT_MS = 5000L

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SwipeableQueueItem(
    index: Int,
    trackId: String,
    trackContent: Content<Track>,
    isCurrentTrack: Boolean,
    onPlayTrack: () -> Unit,
    onRemoveTrack: () -> Unit,
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
            trackName = (trackContent as? Content.Resolved)?.data?.name ?: "Track",
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
                QueueTrackItem(
                    index = index,
                    trackContent = trackContent,
                    isCurrentTrack = isCurrentTrack,
                    onClick = onPlayTrack,
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
private fun QueueTrackItem(
    index: Int,
    trackContent: Content<Track>,
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

    val textColor = if (isCurrentTrack) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurface
    }

    when (trackContent) {
        is Content.Loading -> {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(backgroundColor)
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                CircularProgressIndicator(modifier = Modifier.size(24.dp))
            }
        }
        is Content.Error -> {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(backgroundColor)
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = stringResource(R.string.error_loading_track),
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
        is Content.Resolved -> {
            val track = trackContent.data
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .shadow(elevation)
                    .background(backgroundColor)
                    .clickable(onClick = onClick)
                    .padding(horizontal = 16.dp, vertical = 12.dp),
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
                        text = track.name,
                        style = MaterialTheme.typography.bodyLarge,
                        color = textColor,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    ScrollingArtistsRow(
                        artists = track.artists
                    )
                }

                // Duration
                DurationText(
                    durationSeconds = track.durationSeconds,
                    modifier = Modifier.padding(start = 16.dp)
                )
            }
        }
    }
}
