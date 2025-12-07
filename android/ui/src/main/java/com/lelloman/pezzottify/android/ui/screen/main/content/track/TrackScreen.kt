package com.lelloman.pezzottify.android.ui.screen.main.content.track

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
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.ArtistAvatarRow
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import kotlin.math.min

@Composable
fun TrackScreen(trackId: String, navController: NavController) {
    val viewModel = hiltViewModel<TrackScreenViewModel, TrackScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(trackId = trackId, navController = navController) }
    )
    TrackScreenContent(
        state = viewModel.state.collectAsState().value,
        contentResolver = viewModel.contentResolver,
        actions = viewModel,
        onAlbumClick = viewModel::clickOnAlbum,
        onArtistClick = viewModel::clickOnArtist,
        onAlbumImageClick = viewModel::clickOnAlbumImage,
    )
}

@Composable
private fun TrackScreenContent(
    state: TrackScreenState,
    contentResolver: ContentResolver,
    actions: TrackScreenActions,
    onAlbumClick: (String) -> Unit,
    onArtistClick: (String) -> Unit,
    onAlbumImageClick: (String?) -> Unit,
) {
    when {
        state.isLoading -> LoadingScreen()
        state.track != null -> TrackLoadedScreen(
            track = state.track,
            album = state.album,
            currentPlayingTrackId = state.currentPlayingTrackId,
            isAddToQueueMode = state.isAddToQueueMode,
            isLiked = state.isLiked,
            contentResolver = contentResolver,
            actions = actions,
            onAlbumClick = onAlbumClick,
            onArtistClick = onArtistClick,
            onAlbumImageClick = onAlbumImageClick,
        )
    }
}

@Composable
private fun TrackLoadedScreen(
    track: Track,
    album: Album?,
    currentPlayingTrackId: String?,
    isAddToQueueMode: Boolean,
    isLiked: Boolean,
    contentResolver: ContentResolver,
    actions: TrackScreenActions,
    onAlbumClick: (String) -> Unit,
    onArtistClick: (String) -> Unit,
    onAlbumImageClick: (String?) -> Unit,
) {
    val listState = rememberLazyListState()
    val density = LocalDensity.current

    val statusBarHeight = with(density) {
        WindowInsets.statusBars.getTop(this).toDp()
    }

    val maxHeaderHeight = 300.dp
    val minHeaderHeight = 80.dp + statusBarHeight
    val collapseRangeDp = maxHeaderHeight - minHeaderHeight
    val collapseRangePx = with(density) { collapseRangeDp.toPx() }
    val playButtonSize = 56.dp

    val scrollOffsetPx by remember {
        derivedStateOf {
            if (listState.firstVisibleItemIndex == 0) {
                min(listState.firstVisibleItemScrollOffset.toFloat(), collapseRangePx)
            } else {
                collapseRangePx
            }
        }
    }

    val collapseProgress by remember {
        derivedStateOf {
            scrollOffsetPx / collapseRangePx
        }
    }

    val headerHeight by remember {
        derivedStateOf {
            maxHeaderHeight - (collapseRangeDp * collapseProgress)
        }
    }

    val imageAlpha by remember {
        derivedStateOf {
            1f - collapseProgress
        }
    }

    val isPlaying = track.id == currentPlayingTrackId

    Box(modifier = Modifier.fillMaxSize()) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            state = listState
        ) {
            item {
                Spacer(modifier = Modifier.height(maxHeaderHeight + playButtonSize / 2))
            }

            // Duration info
            item {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        painter = painterResource(R.drawable.baseline_access_time_24),
                        contentDescription = null,
                        modifier = Modifier.size(20.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    DurationText(
                        durationSeconds = track.durationSeconds
                    )
                }
            }

            // Artists section
            if (track.artists.isNotEmpty()) {
                item {
                    Text(
                        text = "Artists",
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    ArtistAvatarRow(
                        artistIds = track.artists.map { it.id },
                        contentResolver = contentResolver,
                        onArtistClick = onArtistClick
                    )
                }
            }

            // Album section
            if (album != null) {
                item {
                    Text(
                        text = "Album",
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    AlbumCard(
                        album = album,
                        onClick = { onAlbumClick(album.id) }
                    )
                }
            }

            // Add some bottom padding
            item {
                Spacer(modifier = Modifier.height(32.dp))
            }
        }

        // Collapsing header with album art
        Surface(
            modifier = Modifier
                .fillMaxWidth()
                .height(headerHeight),
            color = MaterialTheme.colorScheme.surface
        ) {
            Box(modifier = Modifier.fillMaxSize()) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .alpha(imageAlpha)
                        .clickable { onAlbumImageClick(album?.imageUrl) }
                ) {
                    NullablePezzottifyImage(
                        url = album?.imageUrl,
                        placeholder = PezzottifyImagePlaceholder.GenericImage,
                        shape = PezzottifyImageShape.FullSize,
                    )
                }

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

                val textColor = lerp(
                    MaterialTheme.colorScheme.onSurface,
                    Color.White,
                    imageAlpha
                )
                val textTopPadding = statusBarHeight * collapseProgress
                Text(
                    text = track.name,
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

        // Floating like button
        IconButton(
            onClick = { actions.clickOnLike() },
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset(y = headerHeight - playButtonSize / 2)
                .padding(end = 80.dp)
                .size(playButtonSize)
        ) {
            Box(contentAlignment = Alignment.Center) {
                Icon(
                    modifier = Modifier.size(playButtonSize),
                    painter = painterResource(R.drawable.baseline_circle_24),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.surfaceVariant,
                )
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

        // Floating play button
        IconButton(
            onClick = { actions.clickOnPlayTrack() },
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset(y = headerHeight - playButtonSize / 2)
                .padding(end = 16.dp)
                .size(playButtonSize)
        ) {
            Box(contentAlignment = Alignment.Center) {
                Icon(
                    modifier = Modifier.size(playButtonSize),
                    painter = painterResource(R.drawable.baseline_circle_24),
                    contentDescription = null,
                    tint = if (isPlaying) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.primary,
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

@Composable
private fun AlbumCard(
    album: Album,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        NullablePezzottifyImage(
            url = album.imageUrl,
            placeholder = PezzottifyImagePlaceholder.GenericImage,
            shape = PezzottifyImageShape.SmallSquare,
            modifier = Modifier
                .size(64.dp)
                .clip(RoundedCornerShape(8.dp))
        )
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = album.name,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )
        }
        Icon(
            painter = painterResource(R.drawable.baseline_chevron_right_24),
            contentDescription = "Go to album",
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
