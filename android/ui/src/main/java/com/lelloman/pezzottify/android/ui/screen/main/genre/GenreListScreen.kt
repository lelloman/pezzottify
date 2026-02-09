package com.lelloman.pezzottify.android.ui.screen.main.genre

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import com.lelloman.pezzottify.android.ui.theme.CornerRadius
import com.lelloman.pezzottify.android.ui.theme.Elevation
import com.lelloman.pezzottify.android.ui.theme.Spacing
import com.lelloman.pezzottify.android.ui.toGenre

@Composable
fun GenreListScreen(
    navController: NavController,
    viewModel: GenreListScreenViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsState()

    LaunchedEffect(Unit) {
        viewModel.events.collect { event ->
            when (event) {
                is GenreListScreenEvents.NavigateToGenre -> navController.toGenre(event.genreName)
                is GenreListScreenEvents.NavigateBack -> navController.popBackStack()
            }
        }
    }

    GenreListScreenContent(
        state = state,
        actions = viewModel,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun GenreListScreenContent(
    state: GenreListScreenState,
    actions: GenreListScreenActions,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.genres_header)) },
                navigationIcon = {
                    IconButton(onClick = actions::goBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.back)
                        )
                    }
                }
            )
        },
        contentWindowInsets = WindowInsets(0.dp)
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
        ) {
            // Search bar
            OutlinedTextField(
                value = state.searchQuery,
                onValueChange = actions::updateSearchQuery,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = Spacing.Medium)
                    .padding(bottom = Spacing.Medium),
                placeholder = { Text(stringResource(R.string.search_genres_placeholder)) },
                leadingIcon = {
                    Icon(
                        imageVector = Icons.Default.Search,
                        contentDescription = null
                    )
                },
                trailingIcon = {
                    if (state.searchQuery.isNotEmpty()) {
                        IconButton(onClick = { actions.updateSearchQuery("") }) {
                            Icon(
                                imageVector = Icons.Default.Clear,
                                contentDescription = stringResource(R.string.clear)
                            )
                        }
                    }
                },
                singleLine = true,
                shape = RoundedCornerShape(CornerRadius.Medium),
            )

            when {
                state.isLoading -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center
                    ) {
                        PezzottifyLoader(size = LoaderSize.FullScreen)
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
                state.genres.isEmpty() -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            text = stringResource(R.string.no_genres_available),
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
                state.filteredGenres.isEmpty() -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            text = stringResource(R.string.no_genres_found),
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
                else -> {
                    LazyVerticalGrid(
                        columns = GridCells.Fixed(2),
                        modifier = Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(
                            horizontal = Spacing.Medium,
                            vertical = Spacing.Small
                        ),
                        horizontalArrangement = Arrangement.spacedBy(Spacing.Small),
                        verticalArrangement = Arrangement.spacedBy(Spacing.Small)
                    ) {
                        items(state.filteredGenres, key = { it.name }) { genre ->
                            GenreCard(
                                genre = genre,
                                onClick = { actions.clickOnGenre(genre.name) }
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun GenreCard(
    genre: GenreListItemState,
    onClick: () -> Unit,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        shape = RoundedCornerShape(CornerRadius.Medium),
        elevation = CardDefaults.cardElevation(defaultElevation = Elevation.Small),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(Spacing.Medium)
        ) {
            Text(
                text = genre.name,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(modifier = Modifier.height(Spacing.ExtraSmall))
            Text(
                text = formatTrackCount(genre.trackCount),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

private fun formatTrackCount(count: Int): String {
    return when {
        count == 1 -> "1 track"
        count > 1 -> "$count tracks"
        else -> ""
    }
}
