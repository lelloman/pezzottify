package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AssistChip
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SearchBar
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
import com.lelloman.pezzottify.android.ui.screen.main.home.ResolvedRecentlyViewedContent
import com.lelloman.pezzottify.android.ui.theme.ComponentSize
import com.lelloman.pezzottify.android.ui.theme.CornerRadius
import com.lelloman.pezzottify.android.ui.theme.Elevation
import com.lelloman.pezzottify.android.ui.theme.Spacing
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toWhatsNew
import com.lelloman.pezzottify.android.ui.toArtist
import com.lelloman.pezzottify.android.ui.toExternalAlbum
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
                is SearchScreensEvents.NavigateToExternalAlbumScreen -> navController.toExternalAlbum(it.albumId)
                is SearchScreensEvents.NavigateToExternalArtistScreen -> {
                    // Navigate to regular ArtistScreen - server proxy will fetch metadata if needed
                    navController.toArtist(it.artistId)
                }
                is SearchScreensEvents.NavigateToExternalTrackScreen -> {
                    // Navigate to regular TrackScreen - server proxy will fetch metadata if needed
                    navController.toTrack(it.trackId)
                }
                is SearchScreensEvents.ShowRequestError -> snackbarHostState.showSnackbar(context.getString(it.messageRes))
                is SearchScreensEvents.ShowRequestSuccess -> snackbarHostState.showSnackbar(context.getString(R.string.request_added_to_queue))
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
                Row {
                    if (state.query.isNotEmpty()) {
                        IconButton(onClick = { actions.updateQuery("") }) {
                            Icon(Icons.Default.Close, contentDescription = stringResource(R.string.clear_search))
                        }
                    }
                    if (state.canUseExternalSearch) {
                        ExternalSearchToggle(
                            isExternalMode = state.isExternalMode,
                            onToggle = actions::toggleExternalMode
                        )
                    }
                }
            },
            onQueryChange = actions::updateQuery,
            onSearch = actions::updateQuery,
            onActiveChange = {},
            active = false,
        ) {

        }
        SearchFilterChips(
            availableFilters = if (state.isExternalMode && state.canUseExternalSearch) SearchFilter.externalFilters else SearchFilter.catalogFilters,
            selectedFilters = state.selectedFilters,
            onFilterToggled = actions::toggleFilter
        )
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
        } else if (state.isExternalMode && state.canUseExternalSearch) {
            // External search results
            Column(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
            ) {
                // Download limits bar
                state.downloadLimits?.let { limits ->
                    DownloadLimitsBar(limits = limits)
                }

                // External results list
                LazyColumn(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth()
                ) {
                    if (state.externalSearchLoading) {
                        item {
                            LoadingSearchResult()
                        }
                    } else if (state.externalSearchErrorRes != null) {
                        item {
                            ErrorSearchResult()
                        }
                    } else {
                        state.externalResults?.let { results ->
                            items(results, key = { "${it::class.simpleName}_${it.id}" }) { result ->
                                when (result) {
                                    is ExternalSearchResultContent.Album -> ExternalAlbumSearchResult(
                                        result = result,
                                        onClick = { actions.clickOnExternalResult(result) },
                                    )
                                    is ExternalSearchResultContent.Artist -> ExternalArtistSearchResult(
                                        result = result,
                                        onClick = { actions.clickOnExternalResult(result) },
                                    )
                                    is ExternalSearchResultContent.Track -> ExternalTrackSearchResult(
                                        result = result,
                                        onClick = { actions.clickOnExternalResult(result) },
                                    )
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Catalog search results
            LazyColumn(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
            ) {
                state.searchResults?.let { searchResults ->
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
            Box(
                modifier = modifier
                    .fillMaxWidth()
                    .height(ComponentSize.ImageThumbSmall),
                contentAlignment = Alignment.Center
            ) {
                CircularProgressIndicator()
            }
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
            Box(
                modifier = modifier
                    .fillMaxWidth()
                    .height(ComponentSize.ImageThumbSmall),
                contentAlignment = Alignment.Center
            ) {
                CircularProgressIndicator()
            }
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
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
            .clickable { actions.clickOnAlbumSearchResult(searchResult.id) }
    ) {
        NullablePezzottifyImage(url = searchResult.imageUrl)
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
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
            .clickable { actions.clickOnTrackSearchResult(searchResult.id) }
    ) {
        NullablePezzottifyImage(url = searchResult.albumImageUrl)
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
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                text = searchResult.artistNames.joinToString(", "),
                style = MaterialTheme.typography.bodySmall,
                maxLines = 1,
                color = MaterialTheme.colorScheme.onSurfaceVariant
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
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(PezzottifyImageShape.SmallSquare.size)
    ) {
        CircularProgressIndicator()
    }
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
//
//@Composable
//@Preview
//private fun SearchScreenPreview() {
//    val state = remember {
//        mutableStateOf(
//            SearchScreenState(
//                searchResults = listOf(
//                    SearchScreenState.SearchResult.Album(
//                        id = "1",
//                        name = "Album 1",
//                        artistsWithIds = listOf("Artist 1" to "1"),
//                        imageUrl = "",
//                    ),
//                    SearchScreenState.SearchResult.Track(
//                        id = "2",
//                        name = "The Track",
//                        artistsWithIds = listOf("Artist 2" to "2"),
//                        imageUrl = "",
//                        durationSeconds = 120,
//                    ),
//                    SearchScreenState.SearchResult.Artist(
//                        id = "3",
//                        name = "Artist 3",
//                        imageUrl = "",
//                    )
//                )
//            )
//        )
//    }
//
//    PezzottifyTheme {
//        SearchScreenContent(
//            state = state.value,
//            actions = object : SearchScreenActions {
//                override fun updateQuery(query: String) {
//                    state.value = state.value.copy(query = query)
//                }
//            }
//        )
//    }
//}

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
            CircularProgressIndicator(modifier = Modifier.size(24.dp))
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
