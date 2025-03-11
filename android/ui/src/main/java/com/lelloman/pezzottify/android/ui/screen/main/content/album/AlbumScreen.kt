package com.lelloman.pezzottify.android.ui.screen.main.content.album

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.hilt.navigation.compose.hiltViewModel

@Composable
fun AlbumScreen(albumId: String) {
    val viewModel = hiltViewModel<AlbumScreenViewModel, AlbumScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(albumId = albumId) }
    )
    AlbumScreenContent(viewModel.state.collectAsState().value)
}

@Composable
private fun AlbumScreenContent(state: AlbumScreenState) {

}