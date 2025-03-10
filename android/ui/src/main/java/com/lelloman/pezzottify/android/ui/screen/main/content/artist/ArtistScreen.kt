package com.lelloman.pezzottify.android.ui.screen.main.content.artist

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.ui.component.LoadingScreen

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
    }
    Box(modifier = Modifier.fillMaxSize()) {
        Text(state.artist?.name.orEmpty())
    }
}
