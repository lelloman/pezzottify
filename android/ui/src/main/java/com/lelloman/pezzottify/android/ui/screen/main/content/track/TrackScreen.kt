package com.lelloman.pezzottify.android.ui.screen.main.content.track

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.unit.IntOffset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.screen.main.content.ContentOverflowMenu
import com.lelloman.pezzottify.android.ui.screen.main.content.ContentPlaybackActions
import com.lelloman.pezzottify.android.ui.component.ArtistAvatarRow
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.content.Album
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import com.lelloman.pezzottify.android.ui.screen.main.content.EnrichmentInfoBlock
import com.lelloman.pezzottify.android.ui.screen.main.content.EnrichmentStatusIndicator
import com.lelloman.pezzottify.android.ui.screen.main.content.flagBadges
import com.lelloman.pezzottify.android.ui.screen.main.content.formatEnrichmentDate
import com.lelloman.pezzottify.android.ui.screen.main.content.formatLanguageLabel
import com.lelloman.pezzottify.android.ui.screen.main.content.joinMetadataParts
import com.lelloman.pezzottify.android.ui.screen.main.content.titleCase
import kotlin.math.min

@Composable
fun TrackScreen(trackId: String, navController: NavController) {
    val viewModel = hiltViewModel<TrackScreenViewModel, TrackScreenViewModel.Factory>(
        creationCallback = { factory -> factory.create(trackId = trackId, navController = navController) }
    )
    TrackScreenContent(
        state = viewModel.state.collectAsState().value,
        contentResolver = viewModel.contentResolver,
        actions = viewModel,
        onAlbumClick = viewModel::clickOnAlbum,
        onArtistClick = viewModel::clickOnArtist,
        onAlbumImageClick = viewModel::clickOnAlbumImage,
    )
}

@Composable
private fun TrackScreenContent(
    state: TrackScreenState,
    contentResolver: ContentResolver,
    actions: TrackScreenActions,
    onAlbumClick: (String) -> Unit,
    onArtistClick: (String) -> Unit,
    onAlbumImageClick: (String?) -> Unit,
) {
    when {
        state.isLoading -> LoadingScreen()
        state.track != null -> TrackLoadedScreen(
            track = state.track,
            album = state.album,
            currentPlayingTrackId = state.currentPlayingTrackId,
            isLiked = state.isLiked,
            contentResolver = contentResolver,
            actions = actions,
            onAlbumClick = onAlbumClick,
            onArtistClick = onArtistClick,
            onAlbumImageClick = onAlbumImageClick,
        )
    }
}

@Composable
private fun TrackLoadedScreen(
    track: Track,
    album: Album?,
    currentPlayingTrackId: String?,
    isLiked: Boolean,
    contentResolver: ContentResolver,
    actions: TrackScreenActions,
    onAlbumClick: (String) -> Unit,
    onArtistClick: (String) -> Unit,
    onAlbumImageClick: (String?) -> Unit,
) {
    val listState = rememberLazyListState()
    val density = LocalDensity.current

    val statusBarHeight = with(density) {
        WindowInsets.statusBars.getTop(this).toDp()
    }

    val maxHeaderHeight = 300.dp
    // Reserve a 64dp app bar, then 8dp clearance above the floating 56dp actions.
    val minHeaderHeight = statusBarHeight + 64.dp + 8.dp + 56.dp / 2
    val collapseRangeDp = maxHeaderHeight - minHeaderHeight
    val collapseRangePx = with(density) { collapseRangeDp.toPx() }
    val playButtonSize = 56.dp

    val scrollOffsetPx by remember {
        derivedStateOf {
            if (listState.firstVisibleItemIndex == 0) {
                min(listState.firstVisibleItemScrollOffset.toFloat(), collapseRangePx)
            } else {
                collapseRangePx
            }
        }
    }

    val collapseProgress by remember {
        derivedStateOf {
            scrollOffsetPx / collapseRangePx
        }
    }

    val headerHeight by remember {
        derivedStateOf {
            maxHeaderHeight - (collapseRangeDp * collapseProgress)
        }
    }

    val imageAlpha by remember {
        derivedStateOf {
            1f - collapseProgress
        }
    }

    val isPlaying = track.id == currentPlayingTrackId

    Box(modifier = Modifier.fillMaxSize()) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            state = listState
        ) {
            item {
                Spacer(modifier = Modifier.height(maxHeaderHeight + playButtonSize / 2))
            }

            item {
                TrackEnrichmentSection(track = track)
            }

            // Duration info
            item {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        painter = painterResource(R.drawable.baseline_access_time_24),
                        contentDescription = null,
                        modifier = Modifier.size(20.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    DurationText(
                        durationSeconds = track.durationSeconds
                    )
                }
            }

            // Availability status (only show if not available)
            if (track.isUnavailable) {
                item {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = when {
                                track.isFetching -> "⏳"
                                track.isFetchError -> "⚠"
                                else -> "⊘"
                            },
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (track.isFetchError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(20.dp)
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = when {
                                track.isFetching -> stringResource(R.string.track_fetching)
                                track.isFetchError -> stringResource(R.string.track_fetch_error)
                                else -> stringResource(R.string.track_unavailable)
                            },
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (track.isFetchError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }

            // Artists section
            if (track.artists.isNotEmpty()) {
                item {
                    Text(
                        text = stringResource(R.string.artists),
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    ArtistAvatarRow(
                        artistIds = track.artists.map { it.id },
                        contentResolver = contentResolver,
                        onArtistClick = onArtistClick
                    )
                }
            }

            // Album section
            if (album != null) {
                item {
                    Text(
                        text = stringResource(R.string.album),
                        style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                    )
                }
                item {
                    AlbumCard(
                        album = album,
                        onClick = { onAlbumClick(album.id) }
                    )
                }
            }

            // Add some bottom padding
            item {
                Spacer(modifier = Modifier.height(32.dp))
            }
        }

        // Collapsing header with album art
        Surface(
            modifier = Modifier
                .fillMaxWidth()
                .height(headerHeight),
            color = MaterialTheme.colorScheme.surface
        ) {
            Box(modifier = Modifier.fillMaxSize()) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .alpha(imageAlpha)
                        .let { modifier ->
                            val imageUrl = album?.imageUrl
                            if (imageUrl != null) {
                                modifier.clickable { onAlbumImageClick(imageUrl) }
                            } else {
                                modifier
                            }
                        }
                ) {
                    NullablePezzottifyImage(
                        url = album?.imageUrl,
                        placeholder = PezzottifyImagePlaceholder.GenericImage,
                        shape = PezzottifyImageShape.FullSize,
                    )
                }

                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(120.dp)
                        .align(Alignment.BottomStart)
                        .alpha(imageAlpha)
                        .background(
                            Brush.verticalGradient(
                                colors = listOf(
                                    Color.Transparent,
                                    Color.Black.copy(alpha = 0.7f)
                                )
                            )
                        )
                )

                val textColor = lerp(
                    MaterialTheme.colorScheme.onSurface,
                    Color.White,
                    imageAlpha
                )
                val textTopPadding = statusBarHeight * collapseProgress
                Text(
                    text = track.name,
                    style = androidx.compose.ui.text.lerp(
                        MaterialTheme.typography.headlineLarge,
                        MaterialTheme.typography.titleLarge,
                        collapseProgress,
                    ),
                    color = textColor,
                    maxLines = if (collapseProgress > 0.5f) 1 else 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier
                        .align(Alignment.BottomStart)
                        .padding(start = 16.dp, end = 72.dp, bottom = 16.dp + 40.dp * collapseProgress, top = textTopPadding)
                )
            }
        }

        ContentOverflowMenu(
            tint = lerp(MaterialTheme.colorScheme.onSurface, Color.White, imageAlpha),
            modifier = Modifier.align(Alignment.TopEnd)
                .padding(top = statusBarHeight + 8.dp, end = 8.dp),
        ) { dismiss ->
            DropdownMenuItem(
                text = { Text(stringResource(R.string.play_single_track)) },
                enabled = track.isPlayable,
                onClick = { dismiss(); actions.clickOnPlaySingle() },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.add_to_queue)) },
                onClick = { dismiss(); actions.clickOnAddToQueue() },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.go_to_album)) },
                onClick = { dismiss(); onAlbumClick(track.albumId) },
            )
        }

        ContentPlaybackActions(
            isLiked = isLiked,
            onLike = actions::clickOnLike,
            onPlay = actions::clickOnPlayTrack,
            playEnabled = track.isPlayable,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset { IntOffset(0, (headerHeight - playButtonSize / 2).roundToPx()) }
                .padding(end = 16.dp),
        )
    }
}


@Composable
private fun TrackEnrichmentSection(track: Track) {
    val profile = track.enrichment?.profile
    val movement = joinMetadataParts(
        profile?.movementNumber?.let { "No. $it" },
        profile?.movementTitle,
    )
    val facts = listOfNotNull(
        profile?.workTitle?.let { "Work" to it },
        movement?.let { "Movement" to it },
        titleCase(profile?.form)?.let { "Form" to it },
        profile?.keySignature?.let { "Key" to it },
        profile?.opusNumber?.let { "Opus" to it },
        profile?.catalogNumber?.let { "Catalog" to it },
        formatEnrichmentDate(profile?.recordingDate)?.let { "Recorded" to it },
        formatEnrichmentDate(profile?.compositionDate)?.let { "Composed" to it },
        formatLanguageLabel(profile?.language)?.let { "Language" to it },
        titleCase(profile?.trackKind)?.let { "Kind" to it },
    )
    val badges = flagBadges(
        profile?.isInstrumental to "Instrumental",
        profile?.isLive to "Live",
        profile?.isCover to "Cover",
        profile?.isRemix to "Remix",
        profile?.isRemaster to "Remaster",
        profile?.isArrangement to "Arrangement",
    )

    EnrichmentStatusIndicator(
        status = track.enrichmentStatus,
        entityType = "Track",
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
    EnrichmentInfoBlock(
        summary = profile?.summary ?: profile?.notes ?: profile?.performanceContext,
        facts = facts,
        badges = badges,
        tags = track.enrichment?.tags.orEmpty(),
        contributors = track.enrichment?.contributors.orEmpty(),
    )
}

@Composable
private fun AlbumCard(
    album: Album,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        NullablePezzottifyImage(
            url = album.imageUrl,
            placeholder = PezzottifyImagePlaceholder.GenericImage,
            shape = PezzottifyImageShape.SmallSquare,
            modifier = Modifier
                .size(64.dp)
                .clip(RoundedCornerShape(8.dp))
        )
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = album.name,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )
        }
        Icon(
            painter = painterResource(R.drawable.baseline_chevron_right_24),
            contentDescription = stringResource(R.string.go_to_album),
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
