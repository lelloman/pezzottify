package com.lelloman.pezzottify.android.ui.screen.main.content.artist

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
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
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
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import kotlin.math.min
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.AlbumGridItem
import com.lelloman.pezzottify.android.ui.component.ArtistAvatarRow
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.component.SkeletonAlbumGridItem
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toArtist
import kotlinx.coroutines.flow.Flow

@Composable
fun ArtistScreen(
    artistId: String,
    navController: NavController
) {
    val viewModel = hiltViewModel<ArtistScreenViewModel, ArtistScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(artistId = artistId, navController = navController) }
    )
    ArtistScreenContent(
        state = viewModel.state.collectAsState().value,
        contentResolver = viewModel.contentResolver,
        navController = navController,
        actions = viewModel
    )
}

@Composable
private fun ArtistScreenContent(
    state: ArtistScreenState,
    contentResolver: ContentResolver,
    navController: NavController,
    actions: ArtistScreenActions
) {
    when {
        state.isLoading -> LoadingScreen()
        state.isError -> ErrorScreen()
        state.artist != null -> ArtistLoadedScreen(
            artist = state.artist,
            albums = state.albums,
            features = state.features,
            relatedArtists = state.relatedArtists,
            isLiked = state.isLiked,
            contentResolver = contentResolver,
            navController = navController,
            actions = actions
        )
    }
}

@Composable
private fun ErrorScreen() {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(
            text = stringResource(R.string.could_not_load_artist),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.error,
        )
    }
}

@Composable
fun ArtistLoadedScreen(
    artist: Artist,
    albums: List<String>,
    features: List<String>,
    relatedArtists: List<String>,
    isLiked: Boolean,
    contentResolver: ContentResolver,
    navController: NavController,
    actions: ArtistScreenActions
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
    val likeButtonSize = 56.dp

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
            // Spacer for header (extra space for like button overlap)
            item {
                Spacer(modifier = Modifier.height(maxHeaderHeight + likeButtonSize / 2))
            }

            // Related Artists
            if (relatedArtists.isNotEmpty()) {
                item {
                    Text(
                        text = stringResource(R.string.related_artists),
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    ArtistAvatarRow(
                        artistIds = relatedArtists,
                        contentResolver = contentResolver,
                        onArtistClick = { navController.toArtist(it) }
                    )
                }
            }

            // Albums
            if (albums.isNotEmpty()) {
                item {
                    Text(
                        text = stringResource(R.string.albums),
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                albumGridItems(
                    albumIds = albums,
                    keyPrefix = "album",
                    contentResolver = contentResolver,
                    navController = navController
                )
            }

            // Features
            if (features.isNotEmpty()) {
                item {
                    Text(
                        text = stringResource(R.string.features),
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                albumGridItems(
                    albumIds = features,
                    keyPrefix = "feature",
                    contentResolver = contentResolver,
                    navController = navController
                )
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
                // Artist image with fade out
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .alpha(imageAlpha)
                        .let { modifier ->
                            if (artist.imageUrl != null) {
                                modifier.clickable { actions.clickOnArtistImage(artist.imageUrl) }
                            } else {
                                modifier
                            }
                        }
                ) {
                    NullablePezzottifyImage(
                        url = artist.imageUrl,
                        placeholder = PezzottifyImagePlaceholder.Head,
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

                // Artist name - color transitions from white (over image) to onSurface (collapsed)
                val textColor = lerp(
                    MaterialTheme.colorScheme.onSurface,
                    Color.White,
                    imageAlpha
                )
                // Top padding increases as header collapses to stay below status bar
                val textTopPadding = statusBarHeight * collapseProgress
                Text(
                    text = artist.name,
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

        // Floating like button - positioned at bottom-right of header, straddling the boundary
        IconButton(
            onClick = { actions.clickOnLike() },
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset(y = headerHeight - likeButtonSize / 2)
                .padding(end = 16.dp)
                .size(likeButtonSize)
        ) {
            Box(contentAlignment = Alignment.Center) {
                // Background circle
                Icon(
                    modifier = Modifier.size(likeButtonSize),
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
    }
}

/**
 * Emits lazy album grid items into a LazyListScope.
 * Each item is a row of 2 albums, loaded lazily as they scroll into view.
 */
private fun LazyListScope.albumGridItems(
    albumIds: List<String>,
    keyPrefix: String,
    contentResolver: ContentResolver,
    navController: NavController
) {
    val columnsPerRow = 2
    val rows = albumIds.chunked(columnsPerRow)

    items(
        items = rows,
        key = { row -> "$keyPrefix-${row.first()}" }
    ) { rowAlbumIds ->
        AlbumGridRow(
            albumIds = rowAlbumIds,
            columnsPerRow = columnsPerRow,
            contentResolver = contentResolver,
            navController = navController
        )
    }
}

@Composable
private fun AlbumGridRow(
    albumIds: List<String>,
    columnsPerRow: Int,
    contentResolver: ContentResolver,
    navController: NavController
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp)
    ) {
        for (i in 0 until columnsPerRow) {
            val albumId = albumIds.getOrNull(i)
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
                            availability = album.data.availability,
                            onClick = { navController.toAlbum(albumId) }
                        )
                    }
                    is Content.Loading, is Content.Error -> {
                        SkeletonAlbumGridItem(modifier = Modifier.weight(1f))
                    }
                }
            } else {
                Spacer(modifier = Modifier.weight(1f))
            }
        }
    }
}

@Composable
@Preview
private fun ArtistScreenPreview() {
    val navController = rememberNavController()
    PezzottifyTheme {
        // Preview would need mocked data
    }
}