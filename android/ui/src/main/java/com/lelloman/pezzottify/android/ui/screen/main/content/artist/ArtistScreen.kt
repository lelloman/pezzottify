package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.PezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.content.Artist
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun ArtistScreen(artistId: String) {
    val viewModel = hiltViewModel<ArtistScreenViewModel, ArtistScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(artistId = artistId) }
    )
    ArtistScreenContent(viewModel.state.collectAsState().value)
}

@Composable
private fun ArtistScreenContent(
    state: ArtistScreenState
) {
    when {
        state.isLoading -> LoadingScreen()
        state.artist != null -> ArtistLoadedScreen(state.artist)
    }
}

@Composable
fun ArtistLoadedScreen(artist: Artist) {
    Column(modifier = Modifier.fillMaxSize()) {
        PezzottifyImage(
            url = "",
            placeholder = PezzottifyImagePlaceholder.Head,
            shape = PezzottifyImageShape.FullWidthPoster,
        )
        Text(artist.name, style = MaterialTheme.typography.headlineLarge)
    }
}

@Composable
@Preview
private fun ArtistScreenPreview() {
    val state = ArtistScreenState(
        artist = Artist(
            id = "asd",
            name = "Pippo Franco",
        )
    )
    PezzottifyTheme {
        ArtistScreenContent(state)
    }
}