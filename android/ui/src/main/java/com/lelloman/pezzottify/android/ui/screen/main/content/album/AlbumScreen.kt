package com.lelloman.pezzottify.android.ui.screen.main.content.album

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
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlin.math.min
import kotlinx.coroutines.launch
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.ArtistAvatarRow
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.toArtist

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

@Composable
private fun AlbumScreenContent(
    state: AlbumScreenState,
    contentResolver: ContentResolver,
    navController: NavController,
    actions: AlbumScreenActions
) {
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    val showAddedToQueueSnackbar: (String) -> Unit = { message ->
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
                    isAddToQueueMode = state.isAddToQueueMode,
                    isLiked = state.isLiked,
                    contentResolver = contentResolver,
                    actions = actions,
                    onAddedToQueue = showAddedToQueueSnackbar,
                    onArtistClick = { navController.toArtist(it) },
                )
            }
        }
    }
}

@Composable
fun AlbumLoadedScreen(
    album: Album,
    tracks: List<kotlinx.coroutines.flow.Flow<Content<Track>>>?,
    currentPlayingTrackId: String?,
    isAddToQueueMode: Boolean,
    isLiked: Boolean,
    contentResolver: ContentResolver,
    actions: AlbumScreenActions,
    onAddedToQueue: (String) -> Unit = {},
    onArtistClick: (String) -> Unit = {},
) {
    val listState = rememberLazyListState()
    val density = LocalDensity.current

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
                        text = "Artists",
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
            }

            // Track list
            tracks?.let { trackFlows ->
                items(trackFlows) { trackFlow ->
                    when (val track = trackFlow.collectAsState(initial = null).value) {
                        is Content.Resolved -> TrackItem(
                            track = track.data,
                            isPlaying = track.data.id == currentPlayingTrackId,
                            isAddToQueueMode = isAddToQueueMode,
                            actions = actions,
                            onAddedToQueue = onAddedToQueue,
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
                        .clickable { actions.clickOnAlbumImage(album.imageUrl) }
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

        // Floating like button - positioned to the left of play button
        IconButton(
            onClick = { actions.clickOnLike() },
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
                // Heart icon
                Icon(
                    modifier = Modifier.size(28.dp),
                    painter = painterResource(
                        if (isLiked) R.drawable.baseline_favorite_24
                        else R.drawable.baseline_favorite_border_24
                    ),
                    contentDescription = if (isLiked) "Unlike" else "Like",
                    tint = if (isLiked) Color.Red else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        // Floating play button - positioned at bottom-right of header, straddling the boundary
        IconButton(
            onClick = {
                actions.clickOnPlayAlbum(album.id)
                if (isAddToQueueMode) {
                    onAddedToQueue("Album added to queue")
                }
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
                // Icon on top - play or add-to-queue based on mode
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

@Composable
private fun TrackItem(
    track: Track,
    isPlaying: Boolean,
    isAddToQueueMode: Boolean,
    actions: AlbumScreenActions,
    onAddedToQueue: (String) -> Unit,
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
            .clickable {
                actions.clickOnTrack(track.id)
                if (isAddToQueueMode) {
                    onAddedToQueue("Added to queue")
                }
            }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(
            modifier = Modifier.weight(1f)
        ) {
            Text(
                text = track.name,
                style = MaterialTheme.typography.bodyLarge,
                color = textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            ScrollingArtistsRow(
                artists = track.artists
            )
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
        Text("Error loading track")
    }
}