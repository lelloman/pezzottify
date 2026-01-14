package com.lelloman.pezzottify.android.ui.screen.main.content.album

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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlin.math.min
import kotlinx.coroutines.launch
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.HourglassEmpty
import androidx.compose.material.icons.outlined.CloudDownload
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.ArtistAvatarRow
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.component.bottomsheet.AlbumActionsBottomSheet
import com.lelloman.pezzottify.android.ui.component.bottomsheet.PlaylistPickerBottomSheet
import com.lelloman.pezzottify.android.ui.component.bottomsheet.TrackActionsBottomSheet
import com.lelloman.pezzottify.android.ui.component.dialog.CreatePlaylistDialog
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import com.lelloman.pezzottify.android.ui.toArtist
import com.lelloman.pezzottify.android.ui.toTrack

@Composable
fun AlbumScreen(albumId: String, navController: NavController) {
    val viewModel = hiltViewModel<AlbumScreenViewModel, AlbumScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(albumId = albumId, navController = navController) }
    )
    AlbumScreenContent(
        state = viewModel.state.collectAsState().value,
        contentResolver = viewModel.contentResolver,
        navController = navController,
        actions = viewModel
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AlbumScreenContent(
    state: AlbumScreenState,
    contentResolver: ContentResolver,
    navController: NavController,
    actions: AlbumScreenActions
) {
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    // Bottom sheet states
    val trackSheetState = rememberModalBottomSheetState()
    val albumSheetState = rememberModalBottomSheetState()
    val playlistPickerSheetState = rememberModalBottomSheetState()

    // Selected item for bottom sheets
    var selectedTrack by remember { mutableStateOf<Track?>(null) }
    var selectedTrackIsLiked by remember { mutableStateOf(false) }
    var showAlbumSheet by remember { mutableStateOf(false) }
    var showPlaylistPicker by remember { mutableStateOf(false) }
    var showCreatePlaylistDialog by remember { mutableStateOf(false) }

    // Collect like state for the selected track
    selectedTrack?.let { track ->
        val likeState by actions.getTrackLikeState(track.id).collectAsState(initial = false)
        selectedTrackIsLiked = likeState
    }

    // Track whether we're adding a track or album to playlist
    var pendingAddToPlaylistTrackId by remember { mutableStateOf<String?>(null) }
    var pendingAddToPlaylistAlbumId by remember { mutableStateOf<String?>(null) }

    val showSnackbar: (String) -> Unit = { message ->
        scope.launch {
            snackbarHostState.showSnackbar(message)
        }
    }

    Scaffold(
        snackbarHost = {
            SnackbarHost(
                hostState = snackbarHostState,
                modifier = Modifier.padding(WindowInsets.statusBars.asPaddingValues())
            )
        },
        contentWindowInsets = WindowInsets(0.dp)
    ) { contentPadding ->
        Box(modifier = Modifier.fillMaxSize().padding(contentPadding)) {
            when {
                state.isLoading -> LoadingScreen()
                state.album != null -> AlbumLoadedScreen(
                    album = state.album,
                    tracks = state.tracks,
                    currentPlayingTrackId = state.currentPlayingTrackId,
                    isLiked = state.isLiked,
                    downloadRequestState = state.downloadRequestState,
                    contentResolver = contentResolver,
                    actions = actions,
                    onShowSnackbar = showSnackbar,
                    onArtistClick = { navController.toArtist(it) },
                    onTrackMoreClick = { track -> selectedTrack = track },
                    onAlbumMoreClick = { showAlbumSheet = true },
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
                pendingAddToPlaylistAlbumId = null
                showPlaylistPicker = true
            },
            onViewTrack = {
                navController.toTrack(track.id)
            },
            isLiked = selectedTrackIsLiked,
            onToggleLike = {
                actions.toggleTrackLike(track.id, selectedTrackIsLiked)
            },
        )
    }

    // Album actions bottom sheet
    if (showAlbumSheet && state.album != null) {
        AlbumActionsBottomSheet(
            album = state.album,
            sheetState = albumSheetState,
            onDismiss = { showAlbumSheet = false },
            onPlay = {
                actions.clickOnPlayAlbum(state.album.id)
            },
            onAddToQueue = {
                actions.addAlbumToQueue(state.album.id)
                showSnackbar(context.getString(R.string.added_to_queue))
            },
            onAddToPlaylist = {
                pendingAddToPlaylistTrackId = null
                pendingAddToPlaylistAlbumId = state.album.id
                showPlaylistPicker = true
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
                pendingAddToPlaylistAlbumId = null
            },
            onPlaylistSelected = { playlistId ->
                pendingAddToPlaylistTrackId?.let { trackId ->
                    actions.addTrackToPlaylist(trackId, playlistId)
                    showSnackbar(context.getString(R.string.added_to_playlist))
                }
                pendingAddToPlaylistAlbumId?.let { albumId ->
                    actions.addAlbumToPlaylist(albumId, playlistId)
                    showSnackbar(context.getString(R.string.added_to_playlist))
                }
                showPlaylistPicker = false
                pendingAddToPlaylistTrackId = null
                pendingAddToPlaylistAlbumId = null
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
fun AlbumLoadedScreen(
    album: Album,
    tracks: List<kotlinx.coroutines.flow.Flow<Content<Track>>>?,
    currentPlayingTrackId: String?,
    isLiked: Boolean,
    downloadRequestState: DownloadRequestState,
    contentResolver: ContentResolver,
    actions: AlbumScreenActions,
    onShowSnackbar: (String) -> Unit = {},
    onArtistClick: (String) -> Unit = {},
    onTrackMoreClick: (Track) -> Unit = {},
    onAlbumMoreClick: () -> Unit = {},
) {
    val listState = rememberLazyListState()
    val density = LocalDensity.current
    val context = LocalContext.current

    // Get status bar height for proper inset handling
    val statusBarHeight = with(density) {
        WindowInsets.statusBars.getTop(this).toDp()
    }

    // Define header dimensions
    val maxHeaderHeight = 300.dp
    val minHeaderHeight = 80.dp + statusBarHeight
    val collapseRangeDp = maxHeaderHeight - minHeaderHeight
    val collapseRangePx = with(density) { collapseRangeDp.toPx() }
    val playButtonSize = 56.dp

    // Calculate scroll-based values (in pixels)
    val scrollOffsetPx by remember {
        derivedStateOf {
            if (listState.firstVisibleItemIndex == 0) {
                min(listState.firstVisibleItemScrollOffset.toFloat(), collapseRangePx)
            } else {
                collapseRangePx
            }
        }
    }

    // Calculate collapse progress (0 = expanded, 1 = collapsed)
    val collapseProgress by remember {
        derivedStateOf {
            scrollOffsetPx / collapseRangePx
        }
    }

    // Calculate header height (gradual collapse)
    val headerHeight by remember {
        derivedStateOf {
            maxHeaderHeight - (collapseRangeDp * collapseProgress)
        }
    }

    // Calculate image alpha (fade out as it collapses)
    val imageAlpha by remember {
        derivedStateOf {
            1f - collapseProgress
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        // Scrollable content
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            state = listState
        ) {
            // Spacer for header (extra space for play button overlap)
            item {
                Spacer(modifier = Modifier.height(maxHeaderHeight + playButtonSize / 2))
            }

            // Artists row
            if (album.artistsIds.isNotEmpty()) {
                item {
                    Text(
                        text = stringResource(R.string.artists),
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    ArtistAvatarRow(
                        artistIds = album.artistsIds,
                        contentResolver = contentResolver,
                        onArtistClick = onArtistClick
                    )
                }
                item {
                    Spacer(modifier = Modifier.height(16.dp))
                }
            }

            // Download request section
            if (downloadRequestState !is DownloadRequestState.Hidden) {
                item {
                    DownloadRequestSection(
                        state = downloadRequestState,
                        onRequestDownload = { actions.requestDownload() },
                    )
                }
                item {
                    Spacer(modifier = Modifier.height(16.dp))
                }
            }

            // Track list
            tracks?.let { trackFlows ->
                items(trackFlows) { trackFlow ->
                    when (val track = trackFlow.collectAsState(initial = null).value) {
                        is Content.Resolved -> TrackItem(
                            track = track.data,
                            isPlaying = track.data.id == currentPlayingTrackId,
                            onClick = { onTrackMoreClick(track.data) },
                        )
                        null, is Content.Loading -> LoadingTrackItem()
                        is Content.Error -> ErrorTrackItem()
                    }
                }
            }
        }

        // Collapsing header
        Surface(
            modifier = Modifier
                .fillMaxWidth()
                .height(headerHeight),
            color = MaterialTheme.colorScheme.surface
        ) {
            Box(modifier = Modifier.fillMaxSize()) {
                // Album image with fade out
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .alpha(imageAlpha)
                        .let { modifier ->
                            if (album.imageUrl != null) {
                                modifier.clickable { actions.clickOnAlbumImage(album.imageUrl) }
                            } else {
                                modifier
                            }
                        }
                ) {
                    NullablePezzottifyImage(
                        url = album.imageUrl,
                        placeholder = PezzottifyImagePlaceholder.GenericImage,
                        shape = PezzottifyImageShape.FullSize,
                    )
                }

                // Gradient scrim for text readability (fades with image)
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(120.dp)
                        .align(Alignment.BottomStart)
                        .alpha(imageAlpha)
                        .background(
                            Brush.verticalGradient(
                                colors = listOf(
                                    Color.Transparent,
                                    Color.Black.copy(alpha = 0.7f)
                                )
                            )
                        )
                )

                // Album title - color transitions from white (over image) to onSurface (collapsed)
                val textColor = lerp(
                    MaterialTheme.colorScheme.onSurface,
                    Color.White,
                    imageAlpha
                )
                // Top padding increases as header collapses to stay below status bar
                val textTopPadding = statusBarHeight * collapseProgress
                Text(
                    text = album.name,
                    style = MaterialTheme.typography.headlineLarge,
                    color = textColor,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier
                        .align(Alignment.BottomStart)
                        .padding(start = 16.dp, end = 16.dp, bottom = 16.dp, top = textTopPadding)
                )
            }
        }

        // Floating like button - positioned to the left of other buttons
        IconButton(
            onClick = { actions.clickOnLike() },
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset(y = headerHeight - playButtonSize / 2)
                .padding(end = 144.dp)
                .size(playButtonSize)
        ) {
            Box(contentAlignment = Alignment.Center) {
                // Background circle
                Icon(
                    modifier = Modifier.size(playButtonSize),
                    painter = painterResource(R.drawable.baseline_circle_24),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.surfaceVariant,
                )
                // Heart icon
                Icon(
                    modifier = Modifier.size(28.dp),
                    painter = painterResource(
                        if (isLiked) R.drawable.baseline_favorite_24
                        else R.drawable.baseline_favorite_border_24
                    ),
                    contentDescription = stringResource(if (isLiked) R.string.unlike else R.string.like),
                    tint = if (isLiked) Color.Red else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        // Floating add to queue button
        IconButton(
            onClick = {
                actions.addAlbumToQueue(album.id)
                onShowSnackbar(context.getString(R.string.added_to_queue))
            },
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset(y = headerHeight - playButtonSize / 2)
                .padding(end = 80.dp)
                .size(playButtonSize)
        ) {
            Box(contentAlignment = Alignment.Center) {
                // Background circle
                Icon(
                    modifier = Modifier.size(playButtonSize),
                    painter = painterResource(R.drawable.baseline_circle_24),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.surfaceVariant,
                )
                // Add to queue icon
                Icon(
                    modifier = Modifier.size(28.dp),
                    painter = painterResource(R.drawable.baseline_playlist_add_24),
                    contentDescription = stringResource(R.string.add_to_queue),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        // Floating play button - positioned at bottom-right of header, straddling the boundary
        IconButton(
            onClick = {
                actions.clickOnPlayAlbum(album.id)
            },
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset(y = headerHeight - playButtonSize / 2)
                .padding(end = 16.dp)
                .size(playButtonSize)
        ) {
            Box(contentAlignment = Alignment.Center) {
                // Background circle
                Icon(
                    modifier = Modifier.size(playButtonSize),
                    painter = painterResource(R.drawable.baseline_circle_24),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                )
                // Play icon
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

@Composable
private fun DownloadRequestSection(
    state: DownloadRequestState,
    onRequestDownload: () -> Unit,
) {
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp),
        color = MaterialTheme.colorScheme.surfaceVariant,
        shape = MaterialTheme.shapes.medium,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            when (state) {
                is DownloadRequestState.CanRequest -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.weight(1f),
                    ) {
                        Icon(
                            imageVector = Icons.Outlined.CloudDownload,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(24.dp),
                        )
                        Spacer(modifier = Modifier.size(12.dp))
                        Text(
                            text = stringResource(R.string.album_has_unavailable_tracks),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Spacer(modifier = Modifier.size(12.dp))
                    androidx.compose.material3.Button(
                        onClick = onRequestDownload,
                    ) {
                        Text(stringResource(R.string.request_download))
                    }
                }
                is DownloadRequestState.Requesting -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        PezzottifyLoader(size = LoaderSize.Small)
                        Spacer(modifier = Modifier.size(12.dp))
                        Text(
                            text = stringResource(R.string.requesting_download),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                is DownloadRequestState.Requested -> {
                    val statusText = when (state.status) {
                        RequestStatus.Pending -> {
                            val position = state.queuePosition
                            if (position != null) {
                                stringResource(R.string.download_queued_position, position)
                            } else {
                                stringResource(R.string.download_requested)
                            }
                        }
                        RequestStatus.InProgress -> {
                            val progress = state.progress
                            if (progress != null) {
                                stringResource(R.string.downloading_progress, progress.completed, progress.total)
                            } else {
                                stringResource(R.string.downloading)
                            }
                        }
                        RequestStatus.Completed -> stringResource(R.string.download_completed)
                        RequestStatus.Failed -> stringResource(R.string.download_failed)
                    }
                    val icon = when (state.status) {
                        RequestStatus.Pending -> Icons.Filled.HourglassEmpty
                        RequestStatus.InProgress -> Icons.Filled.Download
                        RequestStatus.Completed -> Icons.Filled.CheckCircle
                        RequestStatus.Failed -> Icons.Filled.ErrorOutline
                    }
                    val tint = when (state.status) {
                        RequestStatus.Completed -> MaterialTheme.colorScheme.primary
                        RequestStatus.Failed -> MaterialTheme.colorScheme.error
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    }
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            imageVector = icon,
                            contentDescription = null,
                            tint = tint,
                            modifier = Modifier.size(24.dp),
                        )
                        Spacer(modifier = Modifier.size(12.dp))
                        Text(
                            text = statusText,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                is DownloadRequestState.Error -> {
                    val errorMessage = when (state.errorType) {
                        DownloadRequestErrorType.Network -> stringResource(R.string.download_request_error_network)
                        DownloadRequestErrorType.Unauthorized -> stringResource(R.string.download_request_error_unauthorized)
                        DownloadRequestErrorType.NotFound -> stringResource(R.string.download_request_error_not_found)
                        DownloadRequestErrorType.Unknown -> stringResource(R.string.download_request_error_unknown)
                    }
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.weight(1f),
                    ) {
                        Icon(
                            imageVector = Icons.Filled.ErrorOutline,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.error,
                            modifier = Modifier.size(24.dp),
                        )
                        Spacer(modifier = Modifier.size(12.dp))
                        Text(
                            text = errorMessage,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                    Spacer(modifier = Modifier.size(12.dp))
                    androidx.compose.material3.TextButton(
                        onClick = onRequestDownload,
                    ) {
                        Text(stringResource(R.string.retry))
                    }
                }
                is DownloadRequestState.Hidden -> {
                    // Should not happen as we check before rendering
                }
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
        Text(stringResource(R.string.error_loading_track))
    }
}
