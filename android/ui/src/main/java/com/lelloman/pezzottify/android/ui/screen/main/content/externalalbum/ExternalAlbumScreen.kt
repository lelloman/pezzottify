package com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.outlined.Album
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material.icons.outlined.HourglassEmpty
import androidx.compose.material3.Button
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape

@Composable
fun ExternalAlbumScreen(albumId: String, navController: NavController) {
    val viewModel = hiltViewModel<ExternalAlbumScreenViewModel, ExternalAlbumScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(albumId = albumId, navController = navController) }
    )
    val state by viewModel.state.collectAsState()

    ExternalAlbumScreenContent(
        state = state,
        actions = viewModel,
    )
}

@Composable
private fun ExternalAlbumScreenContent(
    state: ExternalAlbumScreenState,
    actions: ExternalAlbumScreenActions,
) {
    Scaffold(
        contentWindowInsets = WindowInsets(0.dp)
    ) { contentPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(contentPadding)
        ) {
            when {
                state.isLoading -> LoadingScreen()
                (state.errorRes != null || state.errorMessage != null) && state.album == null -> ErrorScreen(
                    errorMessage = state.errorMessage ?: stringResource(state.errorRes ?: R.string.unknown_error),
                    onRetry = actions::retry,
                )
                state.album != null -> AlbumLoadedScreen(
                    album = state.album,
                    requestStatus = state.requestStatus,
                    isRequesting = state.isRequesting,
                    errorRes = state.errorRes,
                    actions = actions,
                )
            }
        }
    }
}

@Composable
private fun ErrorScreen(
    errorMessage: String,
    onRetry: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(
            imageVector = Icons.Outlined.ErrorOutline,
            contentDescription = null,
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.error,
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = errorMessage,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Spacer(modifier = Modifier.height(16.dp))
        Button(onClick = onRetry) {
            Text(stringResource(R.string.retry))
        }
    }
}

@Composable
private fun AlbumLoadedScreen(
    album: UiExternalAlbumWithStatus,
    requestStatus: UiRequestStatus?,
    isRequesting: Boolean,
    errorRes: Int?,
    actions: ExternalAlbumScreenActions,
) {
    LazyColumn(
        modifier = Modifier.fillMaxSize()
    ) {
        // Header with album image
        item {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(300.dp)
            ) {
                // Album image
                if (album.imageUrl != null) {
                    NullablePezzottifyImage(
                        url = album.imageUrl,
                        shape = PezzottifyImageShape.FullSize,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(MaterialTheme.colorScheme.surfaceVariant),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Outlined.Album,
                            contentDescription = null,
                            modifier = Modifier.size(100.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                // Gradient scrim
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(120.dp)
                        .align(Alignment.BottomStart)
                        .background(
                            Brush.verticalGradient(
                                colors = listOf(
                                    Color.Transparent,
                                    Color.Black.copy(alpha = 0.7f)
                                )
                            )
                        )
                )

                // Album title
                Text(
                    text = album.name,
                    style = MaterialTheme.typography.headlineLarge,
                    color = Color.White,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier
                        .align(Alignment.BottomStart)
                        .padding(16.dp)
                )
            }
        }

        // Album info
        item {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp)
            ) {
                // Artist name (clickable)
                Text(
                    text = album.artistName,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.clickable { actions.navigateToArtist() }
                )

                Spacer(modifier = Modifier.height(4.dp))

                // Album metadata
                val metadata = buildString {
                    album.year?.let { append("$it") }
                    album.albumType?.let {
                        if (isNotEmpty()) append(" \u2022 ")
                        append(it.replaceFirstChar { c -> c.uppercase() })
                    }
                    if (isNotEmpty()) append(" \u2022 ")
                    append("${album.totalTracks} ${if (album.totalTracks == 1) "track" else "tracks"}")
                }
                Text(
                    text = metadata,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        // Status / Action section
        item {
            StatusSection(
                album = album,
                requestStatus = requestStatus,
                isRequesting = isRequesting,
                errorRes = errorRes,
                actions = actions,
            )
        }

        // Tracks header
        item {
            Text(
                text = stringResource(R.string.tracks),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
            )
        }

        // Track list
        items(album.tracks) { track ->
            ExternalTrackItem(track = track)
        }

        // Bottom spacing
        item {
            Spacer(modifier = Modifier.height(16.dp))
        }
    }
}

@Composable
private fun StatusSection(
    album: UiExternalAlbumWithStatus,
    requestStatus: UiRequestStatus?,
    isRequesting: Boolean,
    errorRes: Int?,
    actions: ExternalAlbumScreenActions,
) {
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        color = MaterialTheme.colorScheme.surfaceVariant,
        shape = MaterialTheme.shapes.medium,
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            when {
                // Album is in catalog - show button to navigate
                album.inCatalog -> {
                    Icon(
                        imageVector = Icons.Filled.CheckCircle,
                        contentDescription = null,
                        modifier = Modifier.size(48.dp),
                        tint = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = stringResource(R.string.in_your_catalog),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Button(onClick = actions::navigateToCatalogAlbum) {
                        Text(stringResource(R.string.view_album))
                    }
                }

                // Download completed
                requestStatus?.status == UiDownloadStatus.Completed -> {
                    Icon(
                        imageVector = Icons.Filled.CheckCircle,
                        contentDescription = null,
                        modifier = Modifier.size(48.dp),
                        tint = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = stringResource(R.string.download_completed),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Button(onClick = actions::navigateToCatalogAlbum) {
                        Text(stringResource(R.string.view_album))
                    }
                }

                // Download in progress
                requestStatus?.status == UiDownloadStatus.InProgress -> {
                    val progress = requestStatus.progress
                    if (progress != null) {
                        Text(
                            text = stringResource(R.string.downloading),
                            style = MaterialTheme.typography.titleMedium,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        LinearProgressIndicator(
                            progress = { progress.percent },
                            modifier = Modifier.fillMaxWidth(),
                        )
                        Spacer(modifier = Modifier.height(4.dp))
                        Text(
                            text = "${progress.completed}/${progress.total} tracks",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        PezzottifyLoader(size = LoaderSize.Medium)
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = stringResource(R.string.downloading),
                            style = MaterialTheme.typography.titleMedium,
                        )
                    }
                }

                // Download pending
                requestStatus?.status == UiDownloadStatus.Pending -> {
                    Icon(
                        imageVector = Icons.Outlined.HourglassEmpty,
                        contentDescription = null,
                        modifier = Modifier.size(48.dp),
                        tint = MaterialTheme.colorScheme.secondary,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = stringResource(R.string.pending_in_queue),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    requestStatus.queuePosition?.let { position ->
                        Text(
                            text = stringResource(R.string.queue_position, position),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                // Download failed
                requestStatus?.status == UiDownloadStatus.Failed -> {
                    Icon(
                        imageVector = Icons.Outlined.ErrorOutline,
                        contentDescription = null,
                        modifier = Modifier.size(48.dp),
                        tint = MaterialTheme.colorScheme.error,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = stringResource(R.string.download_failed),
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                    requestStatus.errorMessage?.let { errorMsg ->
                        Text(
                            text = errorMsg,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                // Retry waiting
                requestStatus?.status == UiDownloadStatus.RetryWaiting -> {
                    Icon(
                        imageVector = Icons.Outlined.HourglassEmpty,
                        contentDescription = null,
                        modifier = Modifier.size(48.dp),
                        tint = MaterialTheme.colorScheme.secondary,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = stringResource(R.string.retry_waiting),
                        style = MaterialTheme.typography.titleMedium,
                    )
                }

                // No status - show request button
                else -> {
                    if (isRequesting) {
                        PezzottifyLoader(size = LoaderSize.Medium)
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = stringResource(R.string.requesting_download),
                            style = MaterialTheme.typography.titleMedium,
                        )
                    } else {
                        Button(
                            onClick = actions::requestDownload,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Icon(
                                imageVector = Icons.Filled.Download,
                                contentDescription = null,
                                modifier = Modifier.size(24.dp),
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(stringResource(R.string.request_download))
                        }
                    }
                }
            }

            // Show error message if any
            if (errorRes != null && requestStatus == null) {
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = stringResource(errorRes),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}

@Composable
private fun ExternalTrackItem(track: UiExternalTrack) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Track number
        Text(
            text = track.trackNumber.toString(),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(32.dp),
        )

        // Track name
        Text(
            text = track.name,
            style = MaterialTheme.typography.bodyLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )

        // Duration
        track.durationMs?.let { durationMs ->
            val minutes = durationMs / 60000
            val seconds = (durationMs % 60000) / 1000
            Text(
                text = "%d:%02d".format(minutes, seconds),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
