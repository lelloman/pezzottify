package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
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
@Preview
private fun ArtistScreenPreview() {
    val navController = rememberNavController()
    PezzottifyTheme {
        // Preview would need mocked data
    }
}