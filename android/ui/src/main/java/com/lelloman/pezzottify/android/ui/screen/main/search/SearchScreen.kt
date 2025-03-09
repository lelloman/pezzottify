package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.SearchBar
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.ui.component.DurationText
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.SquarePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.SquarePezzottifyImageSize
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.SearchResultContent

@Composable
fun SearchScreen() {
    val viewModel = hiltViewModel<SearchScreenViewModel>()
    SearchScreenContent(
        state = viewModel.state.collectAsState().value,
        actions = viewModel,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreenContent(state: SearchScreenState, actions: SearchScreenActions) {
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
        LazyColumn(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
        ) {
            state.searchResults?.let { searchResults ->
                items(searchResults) { searchResult ->
                    when (val result = searchResult.collectAsState(initial = null).value) {
                        is Content.Resolved -> when (result.data) {
                            is SearchResultContent.Album -> AlbumSearchResult(result.data)
                            is SearchResultContent.Track -> TrackSearchResult(result.data)
                            is SearchResultContent.Artist -> ArtistSearchResult(result.data)
                        }

                        null, is Content.Loading -> LoadingSearchResult()
                        is Content.Error -> ErrorSearchResult()
                    }
                }
            }
        }
    }
}

@Composable
private fun AlbumSearchResult(searchResult: SearchResultContent.Album) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(SquarePezzottifyImageSize.Small.value)
    ) {
        SquarePezzottifyImage(url = searchResult.imageUrl)
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
private fun TrackSearchResult(searchResult: SearchResultContent.Track) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(SquarePezzottifyImageSize.Small.value)
    ) {
        SquarePezzottifyImage(url = "")
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
private fun ArtistSearchResult(searchResult: SearchResultContent.Artist) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp)
            .height(SquarePezzottifyImageSize.Small.value)
    ) {
        SquarePezzottifyImage(
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
            .height(SquarePezzottifyImageSize.Small.value)
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
            .height(SquarePezzottifyImageSize.Small.value)
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
