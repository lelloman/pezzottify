package com.lelloman.pezzottify.android.ui.screen.main.content.album

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.PezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.Track

@Composable
fun AlbumScreen(albumId: String) {
    val viewModel = hiltViewModel<AlbumScreenViewModel, AlbumScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(albumId = albumId) }
    )
    AlbumScreenContent(viewModel.state.collectAsState().value, viewModel)
}

@Composable
private fun AlbumScreenContent(state: AlbumScreenState, actions: AlbumScreenActions) {

    when {
        state.isLoading -> LoadingScreen()
        state.album != null -> AlbumLoadedScreen(state.album, state.tracks, actions)
    }
}

@Composable
fun AlbumLoadedScreen(album: Album, tracks: List<kotlinx.coroutines.flow.Flow<Content<Track>>>?, actions: AlbumScreenActions) {
    Column(modifier = Modifier.fillMaxSize()) {
        PezzottifyImage(
            urls = album.imageUrls,
            placeholder = PezzottifyImagePlaceholder.GenericImage,
            shape = PezzottifyImageShape.FullWidthPoster,
        )
        Text(
            album.name,
            style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.padding(16.dp)
        )

        IconButton(
            onClick = { actions.clickOnPlayAlbum(album.id) },
        ) {
            Icon(
                modifier = Modifier.size(72.dp),
                painter = painterResource(R.drawable.baseline_play_circle_24),
                contentDescription = null,
            )
        }

        tracks?.let { trackFlows ->
            LazyColumn(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
            ) {
                items(trackFlows) { trackFlow ->
                    when (val track = trackFlow.collectAsState(initial = null).value) {
                        is Content.Resolved -> TrackItem(track.data, actions)
                        null, is Content.Loading -> LoadingTrackItem()
                        is Content.Error -> ErrorTrackItem()
                    }
                }
            }
        }
    }
}

@Composable
private fun TrackItem(track: Track, actions: AlbumScreenActions) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
            .clickable { actions.clickOnTrack(track.id) }
    ) {
        PezzottifyImage(
            url = "",
            placeholder = PezzottifyImagePlaceholder.GenericImage,
            shape = PezzottifyImageShape.SmallSquare
        )
        Column(
            modifier = Modifier
                .weight(1f)
                .padding(horizontal = 16.dp)
        ) {
            Text(
                track.name,
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.fillMaxWidth()
            )
            Text(
                track.artistsIds.joinToString(", "),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}

@Composable
private fun LoadingTrackItem() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
    ) {
        CircularProgressIndicator()
    }
}

@Composable
private fun ErrorTrackItem() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
    ) {
        Text("Error loading track")
    }
}