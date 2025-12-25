package com.lelloman.pezzottify.android.ui.screen.main.myrequests

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.HourglassEmpty
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material3.Button
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.AlbumGridItem
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.theme.Spacing

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MyRequestsScreen(
    viewModel: MyRequestsScreenViewModel = hiltViewModel(),
    onNavigateBack: () -> Unit,
    onNavigateToAlbum: (String) -> Unit,
) {
    val state by viewModel.state.collectAsState()

    // Track current time, updated when requests change (on refresh)
    val currentTimeMillis = remember(state.requests) { mutableLongStateOf(System.currentTimeMillis()) }

    LaunchedEffect(Unit) {
        viewModel.events.collect { event ->
            when (event) {
                is MyRequestsScreenEvent.NavigateToAlbum -> onNavigateToAlbum(event.albumId)
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.my_requests_title)) },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = null)
                    }
                },
                actions = {
                    IconButton(onClick = viewModel::refresh) {
                        Icon(Icons.Default.Refresh, contentDescription = null)
                    }
                }
            )
        },
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
    ) { paddingValues ->
        PullToRefreshBox(
            isRefreshing = state.isLoading,
            onRefresh = viewModel::refresh,
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues),
        ) {
            Column(modifier = Modifier.fillMaxSize()) {
                // Limits header
                state.limits?.let { limits ->
                    LimitsHeader(limits = limits)
                }

                // Tab row
                PrimaryTabRow(
                    selectedTabIndex = state.selectedTab.ordinal,
                ) {
                    MyRequestsTab.entries.forEach { tab ->
                        val queueCount = state.requests?.count {
                            it.status == RequestStatus.Pending ||
                            it.status == RequestStatus.InProgress ||
                            it.status == RequestStatus.Failed
                        } ?: 0
                        Tab(
                            selected = state.selectedTab == tab,
                            onClick = { viewModel.onTabSelected(tab) },
                            text = {
                                Text(
                                    text = when (tab) {
                                        MyRequestsTab.Queue -> stringResource(R.string.my_requests_tab_queue, queueCount)
                                        MyRequestsTab.Completed -> stringResource(R.string.my_requests_tab_completed)
                                    }
                                )
                            }
                        )
                    }
                }

                val errorRes = state.errorRes
                when {
                    errorRes != null -> {
                        Box(
                            modifier = Modifier.fillMaxSize(),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = stringResource(errorRes),
                                color = MaterialTheme.colorScheme.error,
                            )
                        }
                    }
                    state.requests == null && state.isLoading -> {
                        PezzottifyLoader(size = LoaderSize.FullScreen)
                    }
                    else -> {
                        when (state.selectedTab) {
                            MyRequestsTab.Queue -> QueueTabContent(
                                requests = state.requests ?: emptyList(),
                                currentTimeMillis = currentTimeMillis.longValue,
                            )
                            MyRequestsTab.Completed -> CompletedTabContent(
                                requests = state.requests ?: emptyList(),
                                contentResolver = viewModel.contentResolver,
                                actions = viewModel,
                                currentTimeMillis = currentTimeMillis.longValue,
                                hasMore = state.hasMoreCompleted,
                                isLoadingMore = state.isLoadingMore,
                                onLoadMore = viewModel::loadMoreCompleted,
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun LimitsHeader(
    limits: UiRequestLimits,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
        horizontalArrangement = Arrangement.SpaceEvenly,
    ) {
        LimitItem(
            label = stringResource(R.string.my_requests_limit_today),
            current = limits.requestsToday,
            max = limits.maxPerDay,
            isAtLimit = limits.isAtDailyLimit,
        )
        LimitItem(
            label = stringResource(R.string.my_requests_limit_in_queue),
            current = limits.inQueue,
            max = limits.maxQueue,
            isAtLimit = limits.isAtQueueLimit,
        )
    }
}

@Composable
private fun LimitItem(
    label: String,
    current: Int,
    max: Int,
    isAtLimit: Boolean,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = "$current / $max",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Bold,
            color = if (isAtLimit) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun QueueTabContent(
    requests: List<UiDownloadRequest>,
    currentTimeMillis: Long,
    modifier: Modifier = Modifier,
) {
    val queueRequests = requests.filter {
        it.status == RequestStatus.Pending ||
        it.status == RequestStatus.InProgress ||
        it.status == RequestStatus.Failed
    }.sortedWith(compareBy(
        { it.status != RequestStatus.InProgress },
        { it.status != RequestStatus.Pending },
        { it.queuePosition ?: Int.MAX_VALUE }
    ))

    if (queueRequests.isEmpty()) {
        Box(
            modifier = modifier.fillMaxSize(),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = stringResource(R.string.my_requests_empty_queue),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    } else {
        LazyColumn(
            modifier = modifier.fillMaxSize(),
        ) {
            items(queueRequests, key = { it.id }) { request ->
                QueueRequestItem(request = request, currentTimeMillis = currentTimeMillis)
            }
        }
    }
}

@Composable
private fun QueueRequestItem(
    request: UiDownloadRequest,
    currentTimeMillis: Long,
    modifier: Modifier = Modifier,
) {
    val isFailed = request.status == RequestStatus.Failed
    val isInProgress = request.status == RequestStatus.InProgress

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Status icon
        Icon(
            imageVector = when {
                isFailed -> Icons.Default.Error
                isInProgress -> Icons.Default.Schedule
                else -> Icons.Default.HourglassEmpty
            },
            contentDescription = when {
                isFailed -> stringResource(R.string.my_requests_status_failed)
                isInProgress -> stringResource(R.string.my_requests_status_in_progress)
                else -> stringResource(R.string.my_requests_status_pending)
            },
            tint = when {
                isFailed -> MaterialTheme.colorScheme.error
                isInProgress -> MaterialTheme.colorScheme.primary
                else -> MaterialTheme.colorScheme.secondary
            },
            modifier = Modifier.size(24.dp),
        )

        Spacer(modifier = Modifier.width(Spacing.Medium))

        Column(modifier = Modifier.weight(1f)) {
            // Album name
            Text(
                text = request.albumName,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            // Artist name
            Text(
                text = request.artistName,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )

            // Time info
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(top = 2.dp),
            ) {
                Text(
                    text = stringResource(R.string.my_requests_requested_time, formatRelativeTime(request.createdAt, currentTimeMillis)),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                request.queuePosition?.let { position ->
                    Text(
                        text = " \u2022 ${stringResource(R.string.my_requests_queue_position, position)}",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            // Progress bar for in-progress items
            if (isInProgress) {
                request.progress?.let { progress ->
                    Spacer(modifier = Modifier.height(4.dp))
                    LinearProgressIndicator(
                        progress = { progress.percent },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Text(
                        text = stringResource(R.string.my_requests_tracks_progress, progress.current, progress.total),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            // Error message for failed items
            if (isFailed) {
                request.errorMessage?.let { error ->
                    Text(
                        text = error,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.error,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

@Composable
private fun CompletedTabContent(
    requests: List<UiDownloadRequest>,
    contentResolver: ContentResolver,
    actions: MyRequestsScreenActions,
    currentTimeMillis: Long,
    hasMore: Boolean,
    isLoadingMore: Boolean,
    onLoadMore: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val completedRequests = requests.filter {
        it.status == RequestStatus.Completed
    }.sortedByDescending { it.completedAt ?: 0L }

    if (completedRequests.isEmpty()) {
        Box(
            modifier = modifier.fillMaxSize(),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = stringResource(R.string.my_requests_empty_completed),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    } else {
        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            modifier = modifier.fillMaxSize(),
            contentPadding = PaddingValues(Spacing.Small),
        ) {
            items(completedRequests, key = { it.id }) { request ->
                CompletedAlbumCard(
                    request = request,
                    contentResolver = contentResolver,
                    onClick = { actions.onRequestClick(request) },
                    currentTimeMillis = currentTimeMillis,
                )
            }
            if (hasMore) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = Spacing.Medium),
                        contentAlignment = Alignment.Center,
                    ) {
                        if (isLoadingMore) {
                            PezzottifyLoader(size = LoaderSize.Small)
                        } else {
                            Button(onClick = onLoadMore) {
                                Text(stringResource(R.string.my_requests_load_more))
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CompletedAlbumCard(
    request: UiDownloadRequest,
    contentResolver: ContentResolver,
    onClick: () -> Unit,
    currentTimeMillis: Long,
    modifier: Modifier = Modifier,
) {
    val catalogId = request.catalogId

    if (catalogId != null) {
        // Try to resolve album from catalog for image
        val albumContent by contentResolver.resolveAlbum(catalogId).collectAsState(initial = Content.Loading(catalogId))

        Column(
            modifier = modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .padding(horizontal = Spacing.Small, vertical = Spacing.Small),
        ) {
            when (val content = albumContent) {
                is Content.Resolved -> {
                    AlbumGridItem(
                        albumName = content.data.name,
                        albumDate = content.data.date,
                        albumCoverUrl = content.data.imageUrl,
                        onClick = onClick,
                    )
                    // Show completion time
                    request.completedAt?.let { completedAt ->
                        Text(
                            text = formatRelativeTime(completedAt, currentTimeMillis),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 12.dp),
                        )
                    }
                }
                else -> {
                    // Fallback to request data while loading or on error
                    AlbumGridItem(
                        albumName = request.albumName,
                        albumDate = request.completedAt ?: request.createdAt,
                        albumCoverUrl = null,
                        onClick = onClick,
                    )
                    request.completedAt?.let { completedAt ->
                        Text(
                            text = formatRelativeTime(completedAt, currentTimeMillis),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 12.dp),
                        )
                    }
                }
            }
        }
    } else {
        // No catalog ID, show basic info
        Column(
            modifier = modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .padding(horizontal = Spacing.Small, vertical = Spacing.Small),
        ) {
            AlbumGridItem(
                albumName = request.albumName,
                albumDate = request.completedAt ?: request.createdAt,
                albumCoverUrl = null,
                onClick = onClick,
            )
            request.completedAt?.let { completedAt ->
                Text(
                    text = formatRelativeTime(completedAt, currentTimeMillis),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 12.dp),
                )
            }
        }
    }
}

@Composable
private fun formatRelativeTime(timestampSeconds: Long, currentTimeMillis: Long): String {
    val timestampMillis = timestampSeconds * 1000
    val diffMillis = currentTimeMillis - timestampMillis

    val minutes = diffMillis / (1000 * 60)
    val hours = diffMillis / (1000 * 60 * 60)
    val days = diffMillis / (1000 * 60 * 60 * 24)
    val months = days / 30

    return when {
        minutes < 1 -> stringResource(R.string.my_requests_time_just_now)
        minutes < 60 -> stringResource(R.string.my_requests_time_minutes_ago, minutes.toInt())
        hours < 24 -> stringResource(R.string.my_requests_time_hours_ago, hours.toInt())
        days < 30 -> stringResource(R.string.my_requests_time_days_ago, days.toInt())
        months < 12 -> stringResource(R.string.my_requests_time_months_ago, months.toInt())
        else -> stringResource(R.string.my_requests_time_years_ago, (months / 12).toInt())
    }
}
