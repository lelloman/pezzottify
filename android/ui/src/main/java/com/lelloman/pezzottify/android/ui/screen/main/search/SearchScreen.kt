package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SearchBar
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
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
    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                is SearchScreensEvents.NavigateToArtistScreen -> navController.toArtist(it.artistId)
                is SearchScreensEvents.NavigateToAlbumScreen -> navController.toAlbum(it.albumId)
                is SearchScreensEvents.NavigateToTrackScreen -> navController.toTrack(it.trackId)
            }
        }
    }
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
            trailingIcon = {
                if (state.query.isNotEmpty()) {
                    IconButton(onClick = { actions.updateQuery("") }) {
                        Icon(Icons.Default.Close, contentDescription = "")
                    }
                }
            },
            onQueryChange = actions::updateQuery,
            onSearch = actions::updateQuery,
            onActiveChange = {},
            active = false,
        ) {

        }
        if (state.query.isEmpty()) {
            RecentlyViewedSection(
                modifier = Modifier.weight(1f),
                recentlyViewedContent = state.recentlyViewedContent,
                actions = actions
            )
        } else {
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
        Column(modifier = Modifier.weight(1f)) {
            Text(
                searchResult.name, modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp)
            )
            Text(
                searchResult.artistsIds.joinToString(", "), modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp)
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
        NullablePezzottifyImage(url = null)
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight(),
        ) {
            Text(
                searchResult.name, modifier = Modifier
                    .fillMaxWidth()
            )
            Text(
                searchResult.artistsIds.joinToString(", "), modifier = Modifier
                    .fillMaxWidth()
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
            .height(PezzottifyImageShape.SmallSquare.size)
            .clickable { actions.clickOnArtistSearchResult(searchResult.id) }
    ) {
        NullablePezzottifyImage(
            url = searchResult.imageUrl,
            placeholder = PezzottifyImagePlaceholder.Head
        )
        Text(
            searchResult.name, modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp)
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
