package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AssistChip
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SearchBar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.SearchResultContent
import com.lelloman.pezzottify.android.ui.content.AlbumAvailability
import com.lelloman.pezzottify.android.ui.content.TrackAvailability
import com.lelloman.pezzottify.android.ui.screen.main.home.ResolvedRecentlyViewedContent
import com.lelloman.pezzottify.android.ui.theme.ComponentSize
import com.lelloman.pezzottify.android.ui.theme.CornerRadius
import com.lelloman.pezzottify.android.ui.theme.Elevation
import com.lelloman.pezzottify.android.ui.theme.Spacing
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toWhatsNew
import com.lelloman.pezzottify.android.ui.toArtist
import com.lelloman.pezzottify.android.ui.toTrack
import kotlinx.coroutines.flow.Flow

@Composable
fun SearchScreen(navController: NavController) {
    val viewModel = hiltViewModel<SearchScreenViewModel>()
    SearchScreenContent(
        state = viewModel.state.collectAsState().value,
        actions = viewModel,
        events = viewModel.events,
        navController = navController,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreenContent(
    state: SearchScreenState,
    actions: SearchScreenActions,
    events: Flow<SearchScreensEvents>,
    navController: NavController,
) {
    val snackbarHostState = remember { SnackbarHostState() }
    val context = LocalContext.current

    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                is SearchScreensEvents.NavigateToArtistScreen -> navController.toArtist(it.artistId)
                is SearchScreensEvents.NavigateToAlbumScreen -> navController.toAlbum(it.albumId)
                is SearchScreensEvents.NavigateToTrackScreen -> navController.toTrack(it.trackId)
                is SearchScreensEvents.ShowMessage -> snackbarHostState.showSnackbar(context.getString(it.messageRes))
                is SearchScreensEvents.NavigateToWhatsNewScreen -> navController.toWhatsNew()
            }
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier.fillMaxSize(),
        ) {
            BackHandler(state.query.isNotEmpty()) {
                actions.updateQuery("")
            }
        SearchBar(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            query = state.query,
            leadingIcon = {
                Icon(Icons.Default.Search, contentDescription = null)
            },
            trailingIcon = {
                if (state.query.isNotEmpty()) {
                    IconButton(onClick = { actions.updateQuery("") }) {
                        Icon(Icons.Default.Close, contentDescription = stringResource(R.string.clear_search))
                    }
                }
            },
            onQueryChange = actions::updateQuery,
            onSearch = actions::updateQuery,
            onActiveChange = {},
            active = false,
        ) {

        }
        // Only show filter chips for classic search (not streaming search)
        if (!state.isStreamingSearchEnabled) {
            SearchFilterChips(
                availableFilters = SearchFilter.catalogFilters,
                selectedFilters = state.selectedFilters,
                onFilterToggled = actions::toggleFilter
            )
        }
        if (state.query.isEmpty()) {
            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
            ) {
                // Search history or recently viewed section
                if (!state.searchHistoryItems.isNullOrEmpty()) {
                    SearchHistorySection(
                        modifier = Modifier,
                        searchHistoryItems = state.searchHistoryItems,
                        actions = actions
                    )
                } else if (!state.recentlyViewedContent.isNullOrEmpty()) {
                    RecentlyViewedSection(
                        modifier = Modifier,
                        recentlyViewedContent = state.recentlyViewedContent,
                        actions = actions
                    )
                }

                // What's New section (shown below recently viewed)
                state.whatsNewContent?.let { whatsNew ->
                    if (whatsNew.albums.isNotEmpty()) {
                        WhatsNewSection(
                            whatsNewContent = whatsNew,
                            actions = actions
                        )
                    }
                }

                Spacer(modifier = Modifier.height(Spacing.Large))
            }
        } else {
            // Catalog search results - use streaming or classic based on setting
            if (state.isStreamingSearchEnabled) {
                StreamingSearchResults(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                    sections = state.streamingSections,
                    isLoading = state.isLoading,
                    query = state.query,
                    actions = actions,
                )
            } else {
                LazyColumn(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth()
                ) {
                    if (state.isLoading && state.searchResults == null) {
                        item {
                            SearchLoadingIndicator()
                        }
                    } else {
                        state.searchResults?.let { searchResults ->
                            if (searchResults.isEmpty()) {
                                item {
                                    EmptySearchResults(query = state.query)
                                }
                            } else {
                                items(searchResults) { searchResult ->
                                    when (val result = searchResult.collectAsState(initial = null).value) {
                                        is Content.Resolved -> when (result.data) {
                                            is SearchResultContent.Album -> AlbumSearchResult(result.data, actions)
                                            is SearchResultContent.Track -> TrackSearchResult(result.data, actions)
                                            is SearchResultContent.Artist -> ArtistSearchResult(
                                                result.data,
                                                actions
                                            )
                                        }

                                        null, is Content.Loading -> LoadingSearchResult()
                                        is Content.Error -> ErrorSearchResult()
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        SnackbarHost(
            hostState = snackbarHostState,
            modifier = Modifier.align(Alignment.BottomCenter),
        )
    }
}

@Composable
private fun SearchHistorySection(
    modifier: Modifier,
    searchHistoryItems: List<Flow<Content<SearchHistoryItem>>>,
    actions: SearchScreenActions
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = Spacing.Medium)
            .padding(top = Spacing.Medium)
    ) {
        val maxGroupSize = 2
        searchHistoryItems.forEachGroup(maxGroupSize) { items ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
            ) {
                for (i in 0 until maxGroupSize) {
                    val item = items.getOrNull(i)
                    val itemState = item?.collectAsState(null)
                    itemState?.value?.let {
                        SearchHistoryItemCard(
                            modifier = Modifier.weight(1f),
                            item = it,
                            actions = actions
                        )
                    } ?: run {
                        Spacer(modifier = Modifier.weight(1f))
                    }
                }
            }
            Spacer(modifier = Modifier.height(Spacing.Small))
        }
    }
}

@Composable
private fun SearchHistoryItemCard(
    modifier: Modifier,
    item: Content<SearchHistoryItem>,
    actions: SearchScreenActions
) {
    when (item) {
        is Content.Resolved -> {
            Card(
                modifier = modifier
                    .fillMaxWidth()
                    .clickable {
                        actions.clickOnSearchHistoryItem(
                            item.itemId,
                            item.data.contentType
                        )
                    },
                shape = RoundedCornerShape(CornerRadius.Medium),
                elevation = CardDefaults.cardElevation(
                    defaultElevation = Elevation.Small
                )
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(Spacing.Small),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    NullablePezzottifyImage(
                        url = item.data.contentImageUrl,
                        shape = PezzottifyImageShape.SmallSquare,
                        modifier = Modifier.size(ComponentSize.ImageThumbSmall)
                    )
                    Spacer(modifier = Modifier.width(Spacing.Small))
                    Column(
                        modifier = Modifier.weight(1f)
                    ) {
                        Text(
                            text = item.data.contentName,
                            style = MaterialTheme.typography.titleMedium,
                            maxLines = 1,
                            color = MaterialTheme.colorScheme.onSurface
                        )
                        Text(
                            text = "\"${item.data.query}\"",
                            style = MaterialTheme.typography.bodySmall,
                            maxLines = 1,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }
        }

        is Content.Loading -> {
            PezzottifyLoader(
                height = ComponentSize.ImageThumbSmall,
                modifier = modifier,
            )
        }

        is Content.Error -> {
            Card(
                modifier = modifier.fillMaxWidth(),
                shape = RoundedCornerShape(CornerRadius.Medium),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.errorContainer
                )
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(ComponentSize.ImageThumbSmall)
                        .padding(Spacing.Small),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = ":(",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onErrorContainer
                    )
                }
            }
        }
    }
}

@Composable
private fun RecentlyViewedSection(
    modifier: Modifier,
    recentlyViewedContent: List<Flow<Content<ResolvedRecentlyViewedContent>>>?,
    actions: SearchScreenActions
) {
    if (recentlyViewedContent.isNullOrEmpty()) {
        return
    }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = Spacing.Medium)
            .padding(top = Spacing.Medium)
    ) {
        val maxGroupSize = 2
        recentlyViewedContent.forEachGroup(maxGroupSize) { items ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
            ) {
                for (i in 0 until maxGroupSize) {
                    val item = items.getOrNull(i)
                    val itemState = item?.collectAsState(null)
                    itemState?.value?.let {
                        RecentlyViewedItem(
                            modifier = Modifier.weight(1f),
                            item = it,
                            actions = actions
                        )
                    } ?: run {
                        Spacer(modifier = Modifier.weight(1f))
                    }
                }
            }
            Spacer(modifier = Modifier.height(Spacing.Small))
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

@Composable
private fun RecentlyViewedItem(
    modifier: Modifier,
    item: Content<ResolvedRecentlyViewedContent>,
    actions: SearchScreenActions
) {
    when (item) {
        is Content.Resolved -> {
            Card(
                modifier = modifier
                    .fillMaxWidth()
                    .clickable {
                        actions.clickOnRecentlyViewedItem(
                            item.itemId,
                            item.data.contentType
                        )
                    },
                shape = RoundedCornerShape(CornerRadius.Medium),
                elevation = CardDefaults.cardElevation(
                    defaultElevation = Elevation.Small
                )
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(Spacing.Small),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    NullablePezzottifyImage(
                        url = item.data.contentImageUrl,
                        shape = PezzottifyImageShape.SmallSquare,
                        modifier = Modifier.size(ComponentSize.ImageThumbSmall)
                    )
                    Spacer(modifier = Modifier.width(Spacing.Small))
                    Column(
                        modifier = Modifier.weight(1f)
                    ) {
                        Text(
                            text = item.data.contentName,
                            style = MaterialTheme.typography.titleMedium,
                            maxLines = 1,
                            color = MaterialTheme.colorScheme.onSurface
                        )
                        Text(
                            text = item.data.contentType.name,
                            style = MaterialTheme.typography.bodySmall,
                            maxLines = 1,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }
        }

        is Content.Loading -> {
            PezzottifyLoader(
                height = ComponentSize.ImageThumbSmall,
                modifier = modifier,
            )
        }

        is Content.Error -> {
            Card(
                modifier = modifier.fillMaxWidth(),
                shape = RoundedCornerShape(CornerRadius.Medium),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.errorContainer
                )
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(ComponentSize.ImageThumbSmall)
                        .padding(Spacing.Small),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = ":(",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onErrorContainer
                    )
                }
            }
        }
    }
}

@Composable
private fun AlbumSearchResult(
    searchResult: SearchResultContent.Album,
    actions: SearchScreenActions
) {
    val isUnavailable = searchResult.availability == AlbumAvailability.Missing
    val isPartial = searchResult.availability == AlbumAvailability.Partial

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
            .alpha(if (isUnavailable) 0.5f else 1f)
            .clickable { actions.clickOnAlbumSearchResult(searchResult.id) }
    ) {
        Box {
            NullablePezzottifyImage(url = searchResult.imageUrl)
            if (isPartial || isUnavailable) {
                Box(
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(4.dp)
                        .size(20.dp)
                        .background(
                            color = if (isUnavailable)
                                MaterialTheme.colorScheme.error.copy(alpha = 0.9f)
                            else
                                MaterialTheme.colorScheme.tertiary.copy(alpha = 0.9f),
                            shape = CircleShape
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Warning,
                        contentDescription = if (isUnavailable)
                            stringResource(R.string.album_unavailable)
                        else
                            stringResource(R.string.album_partial),
                        tint = Color.White,
                        modifier = Modifier.size(14.dp)
                    )
                }
            }
        }
        Spacer(modifier = Modifier.width(Spacing.Medium))
        Column(
            modifier = Modifier.weight(1f).fillMaxHeight(),
            verticalArrangement = Arrangement.Center
        ) {
            Text(
                text = searchResult.name,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                text = searchResult.artistNames.joinToString(", "),
                style = MaterialTheme.typography.bodySmall,
                maxLines = 1,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun TrackSearchResult(
    searchResult: SearchResultContent.Track,
    actions: SearchScreenActions
) {
    val textColor = if (searchResult.isUnavailable) {
        MaterialTheme.colorScheme.onSurface.copy(alpha = 0.4f)
    } else {
        MaterialTheme.colorScheme.onSurface
    }
    val secondaryTextColor = if (searchResult.isUnavailable) {
        MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant
    }

    // Pulsing animation for fetching state
    val fetchingAlpha by animateFloatAsState(
        targetValue = if (searchResult.isFetching) 0.4f else 1f,
        animationSpec = if (searchResult.isFetching) {
            infiniteRepeatable(
                animation = tween(durationMillis = 750, easing = LinearEasing),
                repeatMode = RepeatMode.Reverse
            )
        } else {
            tween(durationMillis = 0)
        },
        label = "fetchingAlpha"
    )

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
            .alpha(
                when {
                    searchResult.isFetching -> fetchingAlpha
                    searchResult.isUnavailable -> 0.4f
                    else -> 1f
                }
            )
            .clickable(enabled = searchResult.isPlayable) {
                actions.clickOnTrackSearchResult(searchResult.id)
            }
    ) {
        // Warning icon for fetch error, otherwise album image
        if (searchResult.isFetchError) {
            Box(
                modifier = Modifier.size(PezzottifyImageShape.SmallSquare.size),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = "⚠",
                    style = MaterialTheme.typography.headlineMedium,
                    color = MaterialTheme.colorScheme.error
                )
            }
        } else {
            NullablePezzottifyImage(url = searchResult.albumImageUrl)
        }
        Spacer(modifier = Modifier.width(Spacing.Medium))
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight(),
            verticalArrangement = Arrangement.Center
        ) {
            Text(
                text = searchResult.name,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                color = textColor
            )
            Text(
                text = searchResult.artistNames.joinToString(", "),
                style = MaterialTheme.typography.bodySmall,
                maxLines = 1,
                color = secondaryTextColor
            )
        }
        DurationText(
            searchResult.durationSeconds, modifier = Modifier
                .align(Alignment.CenterVertically)
                .padding(16.dp)
        )
    }
}

@Composable
private fun ArtistSearchResult(
    searchResult: SearchResultContent.Artist,
    actions: SearchScreenActions
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallCircle.size)
            .clickable { actions.clickOnArtistSearchResult(searchResult.id) }
    ) {
        NullablePezzottifyImage(
            url = searchResult.imageUrl,
            shape = PezzottifyImageShape.SmallCircle,
            placeholder = PezzottifyImagePlaceholder.Head
        )
        Spacer(modifier = Modifier.width(Spacing.Medium))
        Text(
            searchResult.name, modifier = Modifier
                .fillMaxWidth()
                .align(Alignment.CenterVertically)
        )
    }
}

@Composable
private fun LoadingSearchResult() {
    PezzottifyLoader(
        height = PezzottifyImageShape.SmallSquare.size,
        modifier = Modifier.padding(8.dp),
    )
}

@Composable
private fun SearchLoadingIndicator() {
    PezzottifyLoader(size = LoaderSize.Section)
}

@Composable
private fun ErrorSearchResult() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
    ) {
        Text("Error")
    }
}

@Composable
private fun EmptySearchResults(query: String) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = Spacing.ExtraExtraLarge),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = stringResource(R.string.no_results_for_query, query),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun WhatsNewSection(
    whatsNewContent: WhatsNewContentState,
    actions: SearchScreenActions
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Spacing.Medium)
            .padding(top = Spacing.Large)
    ) {
        // Header row with title and "See all" button
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = stringResource(R.string.whats_new_header),
                style = MaterialTheme.typography.titleLarge
            )
            Text(
                text = stringResource(R.string.whats_new_see_all),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .clickable { actions.clickOnWhatsNewSeeAll() }
                    .padding(Spacing.Small)
            )
        }

        Spacer(modifier = Modifier.height(Spacing.Medium))

        // Horizontal scroll with albums grouped by batch
        whatsNewContent.albums.forEach { group ->
            // Batch header chip
            AssistChip(
                onClick = { },
                label = {
                    Text(
                        text = group.batchName,
                        style = MaterialTheme.typography.labelMedium
                    )
                },
                modifier = Modifier.padding(bottom = Spacing.Small)
            )

            // Horizontal album row
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
            ) {
                group.albums.forEach { albumFlow ->
                    val albumState = albumFlow.collectAsState(initial = null).value
                    when (albumState) {
                        is Content.Resolved -> WhatsNewAlbumCard(
                            album = albumState.data,
                            onClick = { actions.clickOnWhatsNewAlbum(albumState.data.id) }
                        )
                        is Content.Loading, null -> WhatsNewAlbumCardLoading()
                        is Content.Error -> WhatsNewAlbumCardError()
                    }
                }
            }

            Spacer(modifier = Modifier.height(Spacing.Medium))
        }
    }
}

@Composable
private fun WhatsNewAlbumCard(
    album: WhatsNewAlbumItem,
    onClick: () -> Unit,
) {
    Card(
        modifier = Modifier
            .width(120.dp)
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
            Column(
                modifier = Modifier.padding(Spacing.Small)
            ) {
                Text(
                    text = album.name,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
    }
}

@Composable
private fun WhatsNewAlbumCardLoading() {
    Card(
        modifier = Modifier
            .width(120.dp)
            .height(160.dp),
        shape = RoundedCornerShape(CornerRadius.Small),
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Small)
    ) {
        Box(
            modifier = Modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            PezzottifyLoader(size = LoaderSize.Small)
        }
    }
}

@Composable
private fun WhatsNewAlbumCardError() {
    Card(
        modifier = Modifier
            .width(120.dp)
            .height(160.dp),
        shape = RoundedCornerShape(CornerRadius.Small),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.errorContainer
        )
    ) {
        Box(
            modifier = Modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = ":(",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onErrorContainer
            )
        }
    }
}

// ========== Streaming Search Results Components ==========

@Composable
private fun StreamingSearchResults(
    modifier: Modifier = Modifier,
    sections: List<StreamingSearchSection>,
    isLoading: Boolean,
    query: String,
    actions: SearchScreenActions,
) {
    LazyColumn(modifier = modifier) {
        // Show loading if no sections yet
        if (isLoading && sections.isEmpty()) {
            item {
                SearchLoadingIndicator()
            }
        }

        // Check for empty results (Done section received but no content)
        val hasDone = sections.any { it is StreamingSearchSection.Done }
        val hasContent = sections.any { section ->
            section is StreamingSearchSection.PrimaryMatch ||
            section is StreamingSearchSection.MoreResults ||
            section is StreamingSearchSection.AllResults
        }

        if (hasDone && !hasContent) {
            item {
                EmptySearchResults(query = query)
            }
        }

        // Render each section
        sections.forEach { section ->
            when (section) {
                is StreamingSearchSection.PrimaryMatch -> {
                    item(key = "primary_${section.type}_${section.id}") {
                        PrimaryMatchCard(section, actions)
                    }
                }
                is StreamingSearchSection.PopularTracks -> {
                    item(key = "popular_${section.targetId}") {
                        SectionHeader(text = stringResource(R.string.streaming_search_popular_tracks))
                    }
                    items(
                        items = section.tracks,
                        key = { "popular_track_${it.id}" }
                    ) { track ->
                        StreamingTrackRow(track, actions)
                    }
                }
                is StreamingSearchSection.ArtistAlbums -> {
                    item(key = "albums_${section.targetId}") {
                        SectionHeader(text = stringResource(R.string.streaming_search_albums))
                    }
                    item(key = "albums_row_${section.targetId}") {
                        AlbumsHorizontalRow(section.albums, actions)
                    }
                }
                is StreamingSearchSection.AlbumTracks -> {
                    item(key = "album_tracks_${section.targetId}") {
                        SectionHeader(text = stringResource(R.string.streaming_search_album_tracks))
                    }
                    items(
                        items = section.tracks,
                        key = { "album_track_${it.id}" }
                    ) { track ->
                        StreamingTrackRow(track, actions)
                    }
                }
                is StreamingSearchSection.RelatedArtists -> {
                    item(key = "related_${section.targetId}") {
                        SectionHeader(text = stringResource(R.string.streaming_search_related_artists))
                    }
                    item(key = "related_row_${section.targetId}") {
                        ArtistsHorizontalRow(section.artists, actions)
                    }
                }
                is StreamingSearchSection.MoreResults -> {
                    item(key = "more_results_header") {
                        SectionHeader(text = stringResource(R.string.streaming_search_more_results))
                    }
                    items(
                        items = section.results,
                        key = { "more_result_${getResultId(it)}" }
                    ) { result ->
                        StreamingSearchResultRow(result, actions)
                    }
                }
                is StreamingSearchSection.AllResults -> {
                    items(
                        items = section.results,
                        key = { "result_${getResultId(it)}" }
                    ) { result ->
                        StreamingSearchResultRow(result, actions)
                    }
                }
                is StreamingSearchSection.Done -> {
                    // Could show timing info if desired
                }
            }
        }

        // Show loading indicator at bottom if still streaming
        if (isLoading && sections.isNotEmpty()) {
            item(key = "loading_more") {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(Spacing.Medium),
                    contentAlignment = Alignment.Center
                ) {
                    PezzottifyLoader(size = LoaderSize.Small)
                }
            }
        }
    }
}

private fun getResultId(result: StreamingSearchResult): String = when (result) {
    is StreamingSearchResult.Artist -> "artist_${result.id}"
    is StreamingSearchResult.Album -> "album_${result.id}"
    is StreamingSearchResult.Track -> "track_${result.id}"
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Spacing.Medium)
            .padding(top = Spacing.Large, bottom = Spacing.Small)
    )
}

@Composable
private fun PrimaryMatchCard(
    match: StreamingSearchSection.PrimaryMatch,
    actions: SearchScreenActions
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small)
            .clickable {
                when (match.type) {
                    PrimaryMatchType.Artist -> actions.clickOnArtistSearchResult(match.id)
                    PrimaryMatchType.Album -> actions.clickOnAlbumSearchResult(match.id)
                    PrimaryMatchType.Track -> actions.clickOnTrackSearchResult(match.id)
                }
            },
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Medium),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.primaryContainer
        ),
        shape = RoundedCornerShape(CornerRadius.Medium)
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(Spacing.Medium),
            verticalAlignment = Alignment.CenterVertically
        ) {
            val imageShape = when (match.type) {
                PrimaryMatchType.Artist -> PezzottifyImageShape.FillWidthCircle
                else -> PezzottifyImageShape.FillWidthSquare
            }
            NullablePezzottifyImage(
                url = match.imageUrl,
                shape = imageShape,
                modifier = Modifier.size(ComponentSize.ImageThumbMedium),
                placeholder = if (match.type == PrimaryMatchType.Artist) PezzottifyImagePlaceholder.Head else PezzottifyImagePlaceholder.GenericImage
            )
            Spacer(modifier = Modifier.width(Spacing.Medium))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = match.name,
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
                if (!match.artistNames.isNullOrEmpty()) {
                    Text(
                        text = match.artistNames.joinToString(", "),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.7f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                match.year?.let {
                    Text(
                        text = it.toString(),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.5f)
                    )
                }
            }
        }
    }
}

@Composable
private fun StreamingTrackRow(
    track: StreamingTrackSummary,
    actions: SearchScreenActions
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { actions.clickOnTrackSearchResult(track.id) }
            .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
        verticalAlignment = Alignment.CenterVertically
    ) {
        NullablePezzottifyImage(
            url = track.imageUrl,
            shape = PezzottifyImageShape.SmallSquare,
            modifier = Modifier.size(ComponentSize.ImageThumbSmall)
        )
        Spacer(modifier = Modifier.width(Spacing.Medium))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.name,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                text = track.artistNames.joinToString(", "),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        }
        DurationText(
            durationSeconds = (track.durationMs / 1000).toInt(),
            modifier = Modifier.padding(start = Spacing.Small)
        )
    }
}

@Composable
private fun AlbumsHorizontalRow(
    albums: List<StreamingAlbumSummary>,
    actions: SearchScreenActions
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = Spacing.Medium),
        horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
    ) {
        albums.forEach { album ->
            StreamingAlbumCard(album, actions)
        }
    }
}

@Composable
private fun StreamingAlbumCard(
    album: StreamingAlbumSummary,
    actions: SearchScreenActions
) {
    val isUnavailable = album.availability == AlbumAvailability.Missing
    val isPartial = album.availability == AlbumAvailability.Partial

    Card(
        modifier = Modifier
            .width(120.dp)
            .clickable { actions.clickOnAlbumSearchResult(album.id) },
        shape = RoundedCornerShape(CornerRadius.Small),
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Small)
    ) {
        Box {
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
            if (isPartial || isUnavailable) {
                Box(
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(4.dp)
                        .size(20.dp)
                        .background(
                            color = if (isUnavailable)
                                MaterialTheme.colorScheme.error.copy(alpha = 0.9f)
                            else
                                MaterialTheme.colorScheme.tertiary.copy(alpha = 0.9f),
                            shape = CircleShape
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Warning,
                        contentDescription = if (isUnavailable)
                            stringResource(R.string.album_unavailable)
                        else
                            stringResource(R.string.album_partial),
                        tint = Color.White,
                        modifier = Modifier.size(14.dp)
                    )
                }
            }
            if (isUnavailable) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .aspectRatio(1f)
                        .alpha(0.5f)
                )
            }
        }
        Column(modifier = Modifier.padding(Spacing.Small)) {
            Text(
                text = album.name,
                style = MaterialTheme.typography.titleSmall,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                color = MaterialTheme.colorScheme.onSurface
            )
            album.releaseYear?.let {
                Text(
                    text = it.toString(),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun ArtistsHorizontalRow(
    artists: List<StreamingArtistSummary>,
    actions: SearchScreenActions
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = Spacing.Medium),
        horizontalArrangement = Arrangement.spacedBy(Spacing.Small)
    ) {
        artists.forEach { artist ->
            StreamingArtistCard(artist, actions)
        }
    }
}

@Composable
private fun StreamingArtistCard(
    artist: StreamingArtistSummary,
    actions: SearchScreenActions
) {
    Column(
        modifier = Modifier
            .width(100.dp)
            .clickable { actions.clickOnArtistSearchResult(artist.id) },
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        NullablePezzottifyImage(
            url = artist.imageUrl,
            shape = PezzottifyImageShape.FillWidthCircle,
            placeholder = PezzottifyImagePlaceholder.Head,
            modifier = Modifier.size(80.dp)
        )
        Spacer(modifier = Modifier.height(Spacing.Small))
        Text(
            text = artist.name,
            style = MaterialTheme.typography.bodySmall,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}

@Composable
private fun StreamingSearchResultRow(
    result: StreamingSearchResult,
    actions: SearchScreenActions
) {
    when (result) {
        is StreamingSearchResult.Artist -> {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { actions.clickOnArtistSearchResult(result.id) }
                    .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
                verticalAlignment = Alignment.CenterVertically
            ) {
                NullablePezzottifyImage(
                    url = result.imageUrl,
                    shape = PezzottifyImageShape.SmallCircle,
                    placeholder = PezzottifyImagePlaceholder.Head,
                    modifier = Modifier.size(ComponentSize.ImageThumbSmall)
                )
                Spacer(modifier = Modifier.width(Spacing.Medium))
                Text(
                    text = result.name,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
        is StreamingSearchResult.Album -> {
            val isUnavailable = result.availability == AlbumAvailability.Missing
            val isPartial = result.availability == AlbumAvailability.Partial

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .alpha(if (isUnavailable) 0.5f else 1f)
                    .clickable { actions.clickOnAlbumSearchResult(result.id) }
                    .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box {
                    NullablePezzottifyImage(
                        url = result.imageUrl,
                        shape = PezzottifyImageShape.SmallSquare,
                        modifier = Modifier.size(ComponentSize.ImageThumbSmall)
                    )
                    if (isPartial || isUnavailable) {
                        Box(
                            modifier = Modifier
                                .align(Alignment.TopEnd)
                                .padding(2.dp)
                                .size(16.dp)
                                .background(
                                    color = if (isUnavailable)
                                        MaterialTheme.colorScheme.error.copy(alpha = 0.9f)
                                    else
                                        MaterialTheme.colorScheme.tertiary.copy(alpha = 0.9f),
                                    shape = CircleShape
                                ),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(
                                imageVector = Icons.Default.Warning,
                                contentDescription = if (isUnavailable)
                                    stringResource(R.string.album_unavailable)
                                else
                                    stringResource(R.string.album_partial),
                                tint = Color.White,
                                modifier = Modifier.size(10.dp)
                            )
                        }
                    }
                }
                Spacer(modifier = Modifier.width(Spacing.Medium))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = result.name,
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSurface,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Text(
                        text = result.artistNames.joinToString(", "),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }
        }
        is StreamingSearchResult.Track -> {
            val isUnavailable = result.availability != TrackAvailability.Available

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .alpha(if (isUnavailable) 0.5f else 1f)
                    .clickable(enabled = !isUnavailable) { actions.clickOnTrackSearchResult(result.id) }
                    .padding(horizontal = Spacing.Medium, vertical = Spacing.Small),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box {
                    NullablePezzottifyImage(
                        url = result.imageUrl,
                        shape = PezzottifyImageShape.SmallSquare,
                        modifier = Modifier.size(ComponentSize.ImageThumbSmall)
                    )
                    if (isUnavailable) {
                        Box(
                            modifier = Modifier
                                .align(Alignment.TopEnd)
                                .padding(2.dp)
                                .size(16.dp)
                                .background(
                                    color = MaterialTheme.colorScheme.error.copy(alpha = 0.9f),
                                    shape = CircleShape
                                ),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(
                                imageVector = Icons.Default.Warning,
                                contentDescription = stringResource(R.string.track_unavailable),
                                tint = Color.White,
                                modifier = Modifier.size(10.dp)
                            )
                        }
                    }
                }
                Spacer(modifier = Modifier.width(Spacing.Medium))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = result.name,
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSurface,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Text(
                        text = result.artistNames.joinToString(", "),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                DurationText(
                    durationSeconds = (result.durationMs / 1000).toInt(),
                    modifier = Modifier.padding(start = Spacing.Small)
                )
            }
        }
    }
}
