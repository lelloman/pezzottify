package com.lelloman.pezzottify.android.ui.screen.main.content.album

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.PezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.content.Album

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
        state.album != null -> AlbumLoadedScreen(state.album, actions)
    }
}

@Composable
fun AlbumLoadedScreen(album: Album, actions: AlbumScreenActions) {
    Column(modifier = Modifier.fillMaxSize()) {
        PezzottifyImage(
            url = "",
            placeholder = PezzottifyImagePlaceholder.GenericImage,
            shape = PezzottifyImageShape.FullWidthPoster,
        )
        Text(album.name, style = MaterialTheme.typography.headlineLarge)

        IconButton(
            onClick = { actions.clickOnPlayAlbum(album.id) },
        ) {
            Icon(
                modifier = Modifier.size(72.dp),
                painter = painterResource(R.drawable.baseline_play_circle_24),
                contentDescription = null,
            )
        }
    }
}