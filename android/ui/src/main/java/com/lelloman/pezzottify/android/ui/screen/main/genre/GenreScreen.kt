package com.lelloman.pezzottify.android.ui.screen.main.genre

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import com.lelloman.pezzottify.android.ui.theme.Elevation
import com.lelloman.pezzottify.android.ui.theme.Spacing
import com.lelloman.pezzottify.android.ui.toTrack

@Composable
fun GenreScreen(
    genreName: String,
    navController: NavController,
) {
    val viewModel = hiltViewModel<GenreScreenViewModel, GenreScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(genreName) }
    )
    val state by viewModel.state.collectAsState()

    LaunchedEffect(Unit) {
        viewModel.events.collect { event ->
            when (event) {
                is GenreScreenEvents.NavigateToTrack -> navController.toTrack(event.trackId)
                is GenreScreenEvents.NavigateBack -> navController.popBackStack()
            }
        }
    }

    GenreScreenContent(
        state = state,
        actions = viewModel,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun GenreScreenContent(
    state: GenreScreenState,
    actions: GenreScreenActions,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = state.genreName,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = actions::goBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.back)
                        )
                    }
                }
            )
        },
        contentWindowInsets = WindowInsets(0.dp)
    ) { paddingValues ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
        ) {
            when {
                state.isLoading -> {
                    PezzottifyLoader(size = LoaderSize.FullScreen)
                }
                state.error != null -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            text = state.error,
                            color = MaterialTheme.colorScheme.error
                        )
                    }
                }
                else -> {
                    Column(modifier = Modifier.fillMaxSize()) {
                        if (state.tracks.isNotEmpty()) {
                            ShufflePlayButton(
                                trackCount = state.totalTracks,
                                onClick = actions::clickOnShufflePlay,
                            )
                        }

                        LazyColumn(
                            modifier = Modifier.fillMaxSize(),
                            verticalArrangement = Arrangement.spacedBy(Spacing.ExtraSmall)
                        ) {
                            items(state.tracks, key = { it.id }) { track ->
                                GenreTrackItem(
                                    track = track,
                                    onClick = { actions.clickOnTrack(track.id) }
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ShufflePlayButton(
    trackCount: Int,
    onClick: () -> Unit,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(Spacing.Medium),
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Medium),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(Spacing.Medium),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Button(onClick = onClick) {
                Icon(
                    imageVector = Icons.Default.PlayArrow,
                    contentDescription = null,
                )
                Spacer(modifier = Modifier.width(Spacing.Small))
                Text(
                    text = "Shuffle Play",
                    style = MaterialTheme.typography.titleMedium,
                )
            }
            Spacer(modifier = Modifier.width(Spacing.Medium))
            Text(
                text = formatTrackCount(trackCount),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun GenreTrackItem(
    track: GenreTrackItemState,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small)
            .padding(start = Spacing.Small)
            .padding(end = Spacing.Medium)
            .height(PezzottifyImageShape.SmallSquare.size)
            .padding(end = Spacing.Medium),
        verticalAlignment = Alignment.CenterVertically
    ) {
        NullablePezzottifyImage(
            url = track.albumImageUrl,
            shape = PezzottifyImageShape.SmallSquare,
        )
        Spacer(modifier = Modifier.width(Spacing.Medium))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.name,
                style = MaterialTheme.typography.titleSmall,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = track.artistNames.joinToString(", "),
                style = MaterialTheme.typography.bodySmall,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        DurationText(
            durationSeconds = (track.durationMs / 1000).toInt(),
        )
    }
}

private fun formatTrackCount(count: Int): String {
    return when {
        count == 1 -> "1 track"
        count > 1 -> "$count tracks"
        else -> ""
    }
}
