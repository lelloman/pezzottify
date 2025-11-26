package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.component.AlbumGridItem
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.PezzottifyImage
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
        navController = navController
    )
}

@Composable
private fun ArtistScreenContent(
    state: ArtistScreenState,
    contentResolver: ContentResolver,
    navController: NavController
) {
    when {
        state.isLoading -> LoadingScreen()
        state.artist != null -> ArtistLoadedScreen(
            artist = state.artist,
            albums = state.albums,
            features = state.features,
            relatedArtists = state.relatedArtists,
            contentResolver = contentResolver,
            navController = navController
        )
    }
}

@Composable
fun ArtistLoadedScreen(
    artist: Artist,
    albums: List<String>,
    features: List<String>,
    relatedArtists: List<String>,
    contentResolver: ContentResolver,
    navController: NavController
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        PezzottifyImage(
            url = "",
            placeholder = PezzottifyImagePlaceholder.Head,
            shape = PezzottifyImageShape.FullWidthPoster,
        )
        Text(
            text = artist.name,
            style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.padding(16.dp)
        )

        if (relatedArtists.isNotEmpty()) {
            Text(
                text = "Related Artists",
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
            )
            RelatedArtistsList(
                artistIds = relatedArtists,
                contentResolver = contentResolver,
                navController = navController
            )
        }

        if (albums.isNotEmpty()) {
            Text(
                text = "Albums",
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
            )
            AlbumGrid(
                albumIds = albums,
                contentResolver = contentResolver,
                navController = navController
            )
        }

        if (features.isNotEmpty()) {
            Text(
                text = "Features",
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
            )
            AlbumGrid(
                albumIds = features,
                contentResolver = contentResolver,
                navController = navController
            )
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
                                albumCoverUrl = album.data.imageUrls.firstOrNull() ?: "",
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
                PezzottifyImage(
                    url = artist.data.imageUrls.firstOrNull() ?: "",
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