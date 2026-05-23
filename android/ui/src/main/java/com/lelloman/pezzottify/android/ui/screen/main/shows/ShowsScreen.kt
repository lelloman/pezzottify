package com.lelloman.pezzottify.android.ui.screen.main.shows

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenScaffold

@Composable
fun ShowsScreen(
    navController: NavController,
    viewModel: ShowsScreenViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsState()
    ShowsScreenContent(
        state = state,
        onBack = { if (state.selectedShow != null) viewModel.clearSelection() else navController.popBackStack() },
        onSelectShow = viewModel::selectShow,
        onPlayTracks = viewModel::playShowTracks,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ShowsScreenContent(
    state: ShowsScreenState,
    onBack: () -> Unit,
    onSelectShow: (String) -> Unit,
    onPlayTracks: () -> Unit,
) {
    MainScreenScaffold(
        topBar = {
            TopAppBar(
                title = { Text("Shows") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    if (state.selectedShow != null) {
                        IconButton(onClick = onPlayTracks) {
                            Icon(Icons.Default.PlayArrow, contentDescription = "Play tracks")
                        }
                    }
                }
            )
        }
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .padding(horizontal = 16.dp)
        ) {
            when {
                state.isLoading -> Text("Loading...", modifier = Modifier.padding(16.dp))
                state.error != null -> Text(state.error, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(16.dp))
                state.selectedShow != null -> ShowDetail(state.selectedShow)
                else -> ShowList(state.shows, onSelectShow)
            }
        }
    }
}

@Composable
private fun ShowList(
    shows: List<ShowSummaryItem>,
    onSelectShow: (String) -> Unit,
) {
    if (shows.isEmpty()) {
        Text("No published shows yet.", modifier = Modifier.padding(16.dp))
        return
    }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        items(shows) { show ->
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onSelectShow(show.id) }
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text(show.title, style = MaterialTheme.typography.titleMedium)
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        show.summary,
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = 3,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        "${show.trackCount} tracks · ${show.targetDurationMinutes} min",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun ShowDetail(show: ShowDetailItem) {
    LazyColumn(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Text(show.title, style = MaterialTheme.typography.headlineSmall)
            Spacer(modifier = Modifier.height(8.dp))
            Text(show.summary, style = MaterialTheme.typography.bodyMedium)
            Spacer(modifier = Modifier.height(12.dp))
            TextButton(onClick = {}) { Text("Use the play button to start the music sequence") }
        }
        items(show.segments) { segment ->
            Card(modifier = Modifier.fillMaxWidth()) {
                Row(
                    modifier = Modifier.padding(14.dp),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(segment.kind.uppercase(), style = MaterialTheme.typography.labelSmall)
                    Column(modifier = Modifier.weight(1f)) {
                        Text(segment.title, style = MaterialTheme.typography.bodyLarge)
                        if (!segment.text.isNullOrBlank()) {
                            Text(
                                segment.text,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 4,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
            }
        }
    }
}
