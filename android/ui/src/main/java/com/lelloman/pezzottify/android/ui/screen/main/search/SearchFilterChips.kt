package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

/**
 * A row of filter chips for search.
 * Chips can be selected/deselected to filter search results by type.
 *
 * @param availableFilters The filters to show (varies between catalog and external mode)
 * @param selectedFilters Currently selected filters
 * @param onFilterToggled Called when a filter chip is tapped
 * @param modifier Modifier for the row
 */
@Composable
fun SearchFilterChips(
    availableFilters: List<SearchFilter>,
    selectedFilters: Set<SearchFilter>,
    onFilterToggled: (SearchFilter) -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        availableFilters.forEach { filter ->
            FilterChip(
                selected = selectedFilters.contains(filter),
                onClick = { onFilterToggled(filter) },
                label = { Text(filter.displayName) }
            )
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun SearchFilterChipsPreviewNoneSelected() {
    PezzottifyTheme {
        SearchFilterChips(
            availableFilters = SearchFilter.catalogFilters,
            selectedFilters = emptySet(),
            onFilterToggled = {}
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun SearchFilterChipsPreviewAlbumSelected() {
    PezzottifyTheme {
        SearchFilterChips(
            availableFilters = SearchFilter.catalogFilters,
            selectedFilters = setOf(SearchFilter.Album),
            onFilterToggled = {}
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun SearchFilterChipsPreviewMultipleSelected() {
    PezzottifyTheme {
        SearchFilterChips(
            availableFilters = SearchFilter.catalogFilters,
            selectedFilters = setOf(SearchFilter.Album, SearchFilter.Track),
            onFilterToggled = {}
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun SearchFilterChipsPreviewExternalFilters() {
    PezzottifyTheme {
        SearchFilterChips(
            availableFilters = SearchFilter.externalFilters,
            selectedFilters = setOf(SearchFilter.Album),
            onFilterToggled = {}
        )
    }
}
