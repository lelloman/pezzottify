package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.shape.CircleShape
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import kotlin.math.min
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.AlbumGridItem
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
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
        creationCallback = { factory -> factory.create(artistId = artistId) }
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

    // Define header dimensions
    val maxHeaderHeight = 300.dp
    val minHeaderHeight = 80.dp
    val collapseRange = (maxHeaderHeight - minHeaderHeight).value
    val likeButtonSize = 56.dp

    // Calculate scroll-based values
    val scrollOffset by remember {
        derivedStateOf {
            if (listState.firstVisibleItemIndex == 0) {
                min(listState.firstVisibleItemScrollOffset.toFloat(), collapseRange)
            } else {
                collapseRange
            }
        }
    }

    // Calculate header height (gradual collapse)
    val headerHeight by remember {
        derivedStateOf {
            (maxHeaderHeight.value - scrollOffset).dp
        }
    }

    // Calculate image alpha (fade out as it collapses)
    val imageAlpha by remember {
        derivedStateOf {
            val progress = scrollOffset / collapseRange
            1f - progress
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
                        text = "Related Artists",
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    RelatedArtistsList(
                        artistIds = relatedArtists,
                        contentResolver = contentResolver,
                        navController = navController
                    )
                }
            }

            // Albums
            if (albums.isNotEmpty()) {
                item {
                    Text(
                        text = "Albums",
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    AlbumGrid(
                        albumIds = albums,
                        contentResolver = contentResolver,
                        navController = navController
                    )
                }
            }

            // Features
            if (features.isNotEmpty()) {
                item {
                    Text(
                        text = "Features",
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    AlbumGrid(
                        albumIds = features,
                        contentResolver = contentResolver,
                        navController = navController
                    )
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
                // Artist image with fade out
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .alpha(imageAlpha)
                ) {
                    NullablePezzottifyImage(
                        url = artist.imageUrl,
                        placeholder = PezzottifyImagePlaceholder.Head,
                        shape = PezzottifyImageShape.FullSize,
                    )
                }

                // Gradient scrim for text readability
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(120.dp)
                        .align(Alignment.BottomStart)
                        .background(
                            Brush.verticalGradient(
                                colors = listOf(
                                    Color.Transparent,
                                    Color.Black.copy(alpha = 0.7f)
                                )
                            )
                        )
                )

                // Artist name
                Text(
                    text = artist.name,
                    style = MaterialTheme.typography.headlineLarge,
                    color = Color.White,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier
                        .align(Alignment.BottomStart)
                        .padding(16.dp)
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
                    contentDescription = if (isLiked) "Unlike" else "Like",
                    tint = if (isLiked) Color.Red else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun AlbumGrid(
    albumIds: List<String>,
    contentResolver: ContentResolver,
    navController: NavController
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
private fun <T> List<T>.forEachGroup(maxGroupSize: Int, action: @Composable (List<T>) -> Unit) {
    val nGroups = size / maxGroupSize + (if (size % maxGroupSize > 0) 1 else 0)
    for (i in 0 until nGroups) {
        val start = i * maxGroupSize
        val end = minOf(start + maxGroupSize, size)
        action(subList(start, end))
    }
}

@Composable
private fun RelatedArtistsList(
    artistIds: List<String>,
    contentResolver: ContentResolver,
    navController: NavController
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 16.dp)
    ) {
        artistIds.forEach { artistId ->
            RelatedArtistItem(
                artistId = artistId,
                contentResolver = contentResolver,
                onClick = { navController.toArtist(artistId) }
            )
        }
    }
}

@Composable
private fun RelatedArtistItem(
    artistId: String,
    contentResolver: ContentResolver,
    onClick: () -> Unit
) {
    val artistFlow = contentResolver.resolveArtist(artistId)
    val artistState = artistFlow.collectAsState(Content.Loading(artistId))

    when (val artist = artistState.value) {
        is Content.Resolved -> {
            Column(
                modifier = Modifier
                    .width(96.dp)
                    .clickable(onClick = onClick)
                    .padding(end = 16.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                NullablePezzottifyImage(
                    url = artist.data.imageUrl,
                    shape = PezzottifyImageShape.SmallSquare,
                    placeholder = PezzottifyImagePlaceholder.Head,
                    modifier = Modifier.clip(CircleShape)
                )

                Text(
                    text = artist.data.name,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 8.dp)
                )
            }
        }
        is Content.Loading, is Content.Error -> {
            // Don't show anything for loading or error states
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