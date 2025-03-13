package com.lelloman.pezzottify.android.ui.screen.main.home

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Person
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
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.PezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toArtist
import com.lelloman.pezzottify.android.ui.toProfile
import com.lelloman.pezzottify.android.ui.toTrack
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch


@Composable
fun HomeScreen(navController: NavController) {
    val viewModel = hiltViewModel<HomeScreenViewModel>()
    HomeScreenContent(
        navController = navController,
        actions = viewModel,
        events = viewModel.events,
        state = viewModel.state.collectAsStateWithLifecycle().value,
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun HomeScreenContent(
    navController: NavController,
    events: Flow<HomeScreenEvents>,
    actions: HomeScreenActions,
    state: HomeScreenState,
) {
    val coroutineScope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                HomeScreenEvents.NavigateToProfileScreen -> {
                    navController.toProfile()
                }

                is HomeScreenEvents.NavigateToAlbum -> navController.toAlbum(it.albumId)
                is HomeScreenEvents.NavigateToArtist -> navController.toArtist(it.artistId)
                is HomeScreenEvents.NavigateToTrack -> navController.toTrack(it.trackId)
            }
        }
    }

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.app_name)) },
                actions = {
                    IconButton(onClick = {
                        coroutineScope.launch { actions.clickOnProfile() }
                    }) {
                        Icon(
                            imageVector = Icons.Default.Person,
                            contentDescription = null,
                            tint = MaterialTheme.typography.headlineLarge.color
                        )
                    }
                }
            )
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .padding(innerPadding)
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
        ) {
            state.recentlyViewedContent?.let { recentlyViewedItems ->
                Text(
                    stringResource(R.string.recently_viewed_item_header),
                    style = MaterialTheme.typography.headlineSmall
                )

                val maxGroupSize = 2
                recentlyViewedItems.forEachGroup(maxGroupSize) { items ->
                    Row(modifier = Modifier.fillMaxWidth()) {
                        for (i in 0 until maxGroupSize) {
                            val item = items.getOrNull(i)
                            val itemState = item?.collectAsState(null)
                            itemState?.value?.let {
                                RecentlyViewedItem(
                                    modifier = Modifier
                                        .weight(1f),
                                    item = it,
                                    actions = actions
                                )
                            } ?: run {
                                Spacer(modifier = Modifier.weight(1f))
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun RecentlyViewedItem(
    modifier: Modifier,
    item: Content<ResolvedRecentlyViewedContent>,
    actions: HomeScreenActions
) {
    Box(modifier = modifier) {
        when (item) {
            is Content.Resolved -> {
                Row(
                    modifier = Modifier
                        .fillMaxSize()
                        .clickable {
                            actions.clickOnRecentlyViewedItem(
                                item.itemId,
                                item.data.contentType
                            )
                        }) {
                    PezzottifyImage(url = "", shape = PezzottifyImageShape.SmallSquare)
                    Text(modifier = Modifier.weight(1f), text = item.data.contentName)
                }
            }

            is Content.Loading -> {
                CircularProgressIndicator()
            }

            is Content.Error -> {
                Text(text = ":(")
            }
        }
    }
}

@Composable
@Preview
private fun HomeScreenPreview() {
    val navController = rememberNavController()
    HomeScreen(navController = navController)
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