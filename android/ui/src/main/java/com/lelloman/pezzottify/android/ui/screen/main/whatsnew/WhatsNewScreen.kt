package com.lelloman.pezzottify.android.ui.screen.main.whatsnew

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.theme.CornerRadius
import com.lelloman.pezzottify.android.ui.theme.Elevation
import com.lelloman.pezzottify.android.ui.theme.Spacing
import com.lelloman.pezzottify.android.ui.toAlbum
import kotlinx.coroutines.flow.Flow
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun WhatsNewScreen(
    navController: NavController,
    viewModel: WhatsNewScreenViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsState()

    LaunchedEffect(Unit) {
        viewModel.events.collect { event ->
            when (event) {
                is WhatsNewScreenEvents.NavigateToAlbum -> navController.toAlbum(event.albumId)
                is WhatsNewScreenEvents.NavigateBack -> navController.popBackStack()
            }
        }
    }

    WhatsNewScreenContent(
        state = state,
        actions = viewModel,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun WhatsNewScreenContent(
    state: WhatsNewScreenState,
    actions: WhatsNewScreenActions,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.whats_new_header)) },
                navigationIcon = {
                    IconButton(onClick = actions::goBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.back)
                        )
                    }
                }
            )
        }
    ) { paddingValues ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
        ) {
            when {
                state.isLoading -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center
                    ) {
                        CircularProgressIndicator()
                    }
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
                state.batches.isEmpty() -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            text = "No updates yet",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
                else -> {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.spacedBy(Spacing.Medium)
                    ) {
                        items(state.batches, key = { it.id }) { batch ->
                            BatchCard(
                                batch = batch,
                                onAlbumClick = actions::clickOnAlbum,
                                onToggleExpand = { actions.toggleBatchExpanded(batch.id) }
                            )
                        }
                        item {
                            Spacer(modifier = Modifier.height(Spacing.Large))
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun BatchCard(
    batch: UiBatch,
    onAlbumClick: (String) -> Unit,
    onToggleExpand: () -> Unit,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Spacing.Medium),
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Small),
    ) {
        Column(
            modifier = Modifier.padding(Spacing.Medium)
        ) {
            // Header row
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onToggleExpand),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = batch.name,
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Text(
                        text = formatDate(batch.closedAt),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Icon(
                    imageVector = if (batch.isExpanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                    contentDescription = if (batch.isExpanded) "Collapse" else "Expand",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            // Description if present
            batch.description?.let { desc ->
                Spacer(modifier = Modifier.height(Spacing.Small))
                Text(
                    text = desc,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            // Summary
            Spacer(modifier = Modifier.height(Spacing.Small))
            SummaryRow(batch.summary)

            // Expanded content with albums
            AnimatedVisibility(visible = batch.isExpanded) {
                batch.albums?.let { albumFlows ->
                    Column {
                        Spacer(modifier = Modifier.height(Spacing.Medium))
                        Text(
                            text = "New albums",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        Spacer(modifier = Modifier.height(Spacing.Small))
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .horizontalScroll(rememberScrollState()),
                            horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
                        ) {
                            albumFlows.forEach { albumFlow ->
                                val albumState = albumFlow.collectAsState(initial = null).value
                                when (albumState) {
                                    is Content.Resolved -> AlbumCard(
                                        album = albumState.data,
                                        onClick = { onAlbumClick(albumState.data.id) }
                                    )
                                    is Content.Loading, null -> AlbumCardLoading()
                                    is Content.Error -> AlbumCardError()
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SummaryRow(summary: UiBatchSummary) {
    val parts = mutableListOf<String>()
    if (summary.artistsAdded > 0) parts.add("+${summary.artistsAdded} artists")
    if (summary.albumsAdded > 0) parts.add("+${summary.albumsAdded} albums")
    if (summary.tracksAdded > 0) parts.add("+${summary.tracksAdded} tracks")

    val updateParts = mutableListOf<String>()
    if (summary.artistsUpdated > 0) updateParts.add("${summary.artistsUpdated} artists updated")
    if (summary.albumsUpdated > 0) updateParts.add("${summary.albumsUpdated} albums updated")
    if (summary.tracksUpdated > 0) updateParts.add("${summary.tracksUpdated} tracks updated")

    Column {
        if (parts.isNotEmpty()) {
            Text(
                text = parts.joinToString(" • "),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.primary,
            )
        }
        if (updateParts.isNotEmpty()) {
            Text(
                text = updateParts.joinToString(" • "),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun AlbumCard(
    album: UiWhatsNewAlbum,
    onClick: () -> Unit,
) {
    Card(
        modifier = Modifier
            .width(100.dp)
            .clickable(onClick = onClick),
        shape = RoundedCornerShape(CornerRadius.Small),
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Small)
    ) {
        Column {
            NullablePezzottifyImage(
                url = album.imageUrl,
                shape = PezzottifyImageShape.FullSize,
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(1f)
                    .clip(
                        RoundedCornerShape(
                            topStart = CornerRadius.Small,
                            topEnd = CornerRadius.Small,
                            bottomStart = 0.dp,
                            bottomEnd = 0.dp
                        )
                    )
            )
            Text(
                text = album.name,
                style = MaterialTheme.typography.labelSmall,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(Spacing.ExtraSmall),
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

@Composable
private fun AlbumCardLoading() {
    Card(
        modifier = Modifier
            .width(100.dp)
            .height(130.dp),
        shape = RoundedCornerShape(CornerRadius.Small),
    ) {
        Box(
            modifier = Modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            CircularProgressIndicator(modifier = Modifier.size(20.dp))
        }
    }
}

@Composable
private fun AlbumCardError() {
    Card(
        modifier = Modifier
            .width(100.dp)
            .height(130.dp),
        shape = RoundedCornerShape(CornerRadius.Small),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer)
    ) {
        Box(
            modifier = Modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = ":(",
                color = MaterialTheme.colorScheme.onErrorContainer
            )
        }
    }
}

private fun formatDate(timestamp: Long): String {
    val date = Date(timestamp * 1000) // Convert seconds to milliseconds
    val format = SimpleDateFormat("MMM d, yyyy", Locale.getDefault())
    return format.format(date)
}
