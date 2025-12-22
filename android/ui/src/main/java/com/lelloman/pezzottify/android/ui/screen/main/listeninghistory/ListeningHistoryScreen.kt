package com.lelloman.pezzottify.android.ui.screen.main.listeninghistory

import androidx.compose.foundation.ExperimentalFoundationApi
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Refresh
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.theme.Spacing
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ListeningHistoryScreen(
    viewModel: ListeningHistoryScreenViewModel = hiltViewModel(),
    onNavigateBack: () -> Unit,
    onNavigateToTrack: (String) -> Unit,
) {
    val state by viewModel.state.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.listening_history_title)) },
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
            isRefreshing = state.isLoading && state.events.isEmpty(),
            onRefresh = viewModel::refresh,
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues),
        ) {
            val errorRes = state.errorRes
            when {
                errorRes != null && state.events.isEmpty() -> {
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
                state.events.isEmpty() && !state.isLoading -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = stringResource(R.string.listening_history_empty),
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                else -> {
                    ListeningEventsList(
                        events = state.events,
                        contentResolver = viewModel.contentResolver,
                        isLoading = state.isLoading,
                        hasMorePages = state.hasMorePages,
                        onLoadMore = viewModel::loadMore,
                        onEventClick = { event -> onNavigateToTrack(event.trackId) },
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ListeningEventsList(
    events: List<UiListeningEvent>,
    contentResolver: ContentResolver,
    isLoading: Boolean,
    hasMorePages: Boolean,
    onLoadMore: () -> Unit,
    onEventClick: (UiListeningEvent) -> Unit,
    modifier: Modifier = Modifier,
) {
    val listState = rememberLazyListState()

    // Use snapshotFlow to observe scroll position changes
    LaunchedEffect(listState, hasMorePages, isLoading) {
        snapshotFlow {
            val layoutInfo = listState.layoutInfo
            val lastVisibleItem = layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
            val totalItems = layoutInfo.totalItemsCount
            totalItems > 0 && lastVisibleItem >= totalItems - 5
        }.collect { nearEnd ->
            if (nearEnd && hasMorePages && !isLoading) {
                onLoadMore()
            }
        }
    }

    // Group events by date section
    val groupedEvents = events.groupBy { event -> getDateSection(event.startedAt) }

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
    ) {
        groupedEvents.forEach { (section, sectionEvents) ->
            // Sticky section header
            stickyHeader(key = "header_${section.key}") {
                DateSectionHeader(section)
            }

            // Events in this section
            itemsIndexed(
                items = sectionEvents,
                key = { _, event -> event.id }
            ) { _, event ->
                ListeningEventItem(
                    event = event,
                    contentResolver = contentResolver,
                    onClick = { onEventClick(event) },
                )
            }
        }

        // Loading indicator at bottom - always show when loading more
        if (isLoading && events.isNotEmpty()) {
            item(key = "loading_indicator") {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = Spacing.Large),
                    contentAlignment = Alignment.Center,
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        PezzottifyLoader(size = LoaderSize.Small)
                        Spacer(modifier = Modifier.width(Spacing.Medium))
                        Text(
                            text = stringResource(R.string.loading),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun DateSectionHeader(section: DateSection) {
    val text = when (section) {
        is DateSection.Today -> stringResource(R.string.date_section_today)
        is DateSection.Yesterday -> stringResource(R.string.date_section_yesterday)
        is DateSection.LastSunday -> stringResource(R.string.date_section_last_sunday)
        is DateSection.LastMonday -> stringResource(R.string.date_section_last_monday)
        is DateSection.LastTuesday -> stringResource(R.string.date_section_last_tuesday)
        is DateSection.LastWednesday -> stringResource(R.string.date_section_last_wednesday)
        is DateSection.LastThursday -> stringResource(R.string.date_section_last_thursday)
        is DateSection.LastFriday -> stringResource(R.string.date_section_last_friday)
        is DateSection.LastSaturday -> stringResource(R.string.date_section_last_saturday)
        is DateSection.LastWeek -> stringResource(R.string.date_section_last_week)
        is DateSection.LastMonth -> stringResource(R.string.date_section_last_month)
        is DateSection.MonthsAgo -> if (section.count == 1) {
            stringResource(R.string.date_section_months_ago_one)
        } else {
            stringResource(R.string.date_section_months_ago, section.count)
        }
        is DateSection.YearsAgo -> if (section.count == 1) {
            stringResource(R.string.date_section_years_ago_one)
        } else {
            stringResource(R.string.date_section_years_ago, section.count)
        }
    }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun ListeningEventItem(
    event: UiListeningEvent,
    contentResolver: ContentResolver,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val trackContent by contentResolver.resolveTrack(event.trackId).collectAsState(initial = Content.Loading(event.trackId))

    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            when (val content = trackContent) {
                is Content.Resolved -> {
                    Text(
                        text = content.data.name,
                        style = MaterialTheme.typography.bodyLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    ScrollingArtistsRow(
                        artists = content.data.artists,
                        textStyle = MaterialTheme.typography.bodySmall,
                        textColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                is Content.Loading -> {
                    Text(
                        text = stringResource(R.string.loading),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                is Content.Error -> {
                    Text(
                        text = event.trackId,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }

            Spacer(modifier = Modifier.height(2.dp))

            // Time info row
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = formatTime(event.startedAt),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    text = " \u2022 ",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    text = formatDuration(event.durationSeconds),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                event.clientType?.let { clientType ->
                    Text(
                        text = " \u2022 ",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        text = clientType,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        Spacer(modifier = Modifier.width(Spacing.Small))

        // Completed indicator
        if (event.completed) {
            Icon(
                imageVector = Icons.Default.CheckCircle,
                contentDescription = stringResource(R.string.listening_history_completed),
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(20.dp),
            )
        }
    }
}

private sealed class DateSection(val key: String) {
    data object Today : DateSection("today")
    data object Yesterday : DateSection("yesterday")
    data object LastSunday : DateSection("last_sunday")
    data object LastMonday : DateSection("last_monday")
    data object LastTuesday : DateSection("last_tuesday")
    data object LastWednesday : DateSection("last_wednesday")
    data object LastThursday : DateSection("last_thursday")
    data object LastFriday : DateSection("last_friday")
    data object LastSaturday : DateSection("last_saturday")
    data object LastWeek : DateSection("last_week")
    data object LastMonth : DateSection("last_month")
    data class MonthsAgo(val count: Int) : DateSection("months_ago_$count")
    data class YearsAgo(val count: Int) : DateSection("years_ago_$count")
}

private fun getDateSection(timestampSeconds: Long): DateSection {
    val eventDate = Calendar.getInstance().apply {
        timeInMillis = timestampSeconds * 1000
        set(Calendar.HOUR_OF_DAY, 0)
        set(Calendar.MINUTE, 0)
        set(Calendar.SECOND, 0)
        set(Calendar.MILLISECOND, 0)
    }

    val today = Calendar.getInstance().apply {
        set(Calendar.HOUR_OF_DAY, 0)
        set(Calendar.MINUTE, 0)
        set(Calendar.SECOND, 0)
        set(Calendar.MILLISECOND, 0)
    }

    val daysDiff = ((today.timeInMillis - eventDate.timeInMillis) / (24 * 60 * 60 * 1000)).toInt()

    return when {
        daysDiff == 0 -> DateSection.Today
        daysDiff == 1 -> DateSection.Yesterday
        daysDiff in 2..7 -> {
            when (eventDate.get(Calendar.DAY_OF_WEEK)) {
                Calendar.SUNDAY -> DateSection.LastSunday
                Calendar.MONDAY -> DateSection.LastMonday
                Calendar.TUESDAY -> DateSection.LastTuesday
                Calendar.WEDNESDAY -> DateSection.LastWednesday
                Calendar.THURSDAY -> DateSection.LastThursday
                Calendar.FRIDAY -> DateSection.LastFriday
                Calendar.SATURDAY -> DateSection.LastSaturday
                else -> DateSection.LastWeek // fallback, shouldn't happen
            }
        }
        daysDiff in 8..14 -> DateSection.LastWeek
        daysDiff in 15..30 -> DateSection.LastMonth
        else -> {
            val years = daysDiff / 365
            if (years >= 1) {
                DateSection.YearsAgo(years)
            } else {
                val months = daysDiff / 30
                DateSection.MonthsAgo(months.coerceAtLeast(1))
            }
        }
    }
}

private fun formatTime(timestampSeconds: Long): String {
    val timeFormat = SimpleDateFormat("HH:mm", Locale.getDefault())
    return timeFormat.format(Date(timestampSeconds * 1000))
}

private fun formatDuration(seconds: Int): String {
    val minutes = seconds / 60
    val secs = seconds % 60
    return String.format(Locale.getDefault(), "%d:%02d", minutes, secs)
}
