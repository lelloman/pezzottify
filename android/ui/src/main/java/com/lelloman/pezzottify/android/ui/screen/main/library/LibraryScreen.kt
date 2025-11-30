package com.lelloman.pezzottify.android.ui.screen.main.library

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.component.AlbumGridItem
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.toAlbum

@Composable
fun LibraryScreen(navController: NavController) {
    val viewModel = hiltViewModel<LibraryScreenViewModel>()
    LibraryScreenContent(
        state = viewModel.state.collectAsState().value,
        contentResolver = viewModel.contentResolver,
        navController = navController,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LibraryScreenContent(
    state: LibraryScreenState,
    contentResolver: ContentResolver,
    navController: NavController,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Your albums") }
            )
        }
    ) { paddingValues ->
        Box(modifier = Modifier.padding(paddingValues)) {
            when {
                state.isLoading -> LoadingScreen()
                state.likedAlbumIds.isEmpty() -> EmptyLibraryScreen()
                else -> LibraryLoadedScreen(
                    state = state,
                    contentResolver = contentResolver,
                    navController = navController,
                )
            }
        }
    }
}

@Composable
private fun EmptyLibraryScreen() {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = "No liked albums yet",
                style = MaterialTheme.typography.headlineSmall,
            )
            Text(
                text = "Albums you like will appear here",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
    }
}

@Composable
private fun LibraryLoadedScreen(
    state: LibraryScreenState,
    contentResolver: ContentResolver,
    navController: NavController,
) {
    LazyColumn(
        modifier = Modifier.fillMaxSize()
    ) {
        item {
            AlbumGrid(
                albumIds = state.likedAlbumIds,
                contentResolver = contentResolver,
                navController = navController,
            )
        }
    }
}

@Composable
private fun AlbumGrid(
    albumIds: List<String>,
    contentResolver: ContentResolver,
    navController: NavController,
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
