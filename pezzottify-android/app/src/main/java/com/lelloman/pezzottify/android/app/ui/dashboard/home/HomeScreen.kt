package com.lelloman.pezzottify.android.app.ui.dashboard.home

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Divider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel

@Composable
fun HomeScreen(viewModel: HomeViewModel = hiltViewModel()) {
    val listItems by viewModel.items.collectAsState(emptyList())
    LazyColumn(modifier = Modifier.background(color = Color(0xff00ff00))) {
        itemsIndexed(listItems) { index, item ->
            AlbumListItem(
                viewModel = viewModel,
                listItem = item,
                isLastItem = index == listItems.lastIndex
            )
        }
    }
}

@Composable
fun AlbumListItem(
    viewModel: HomeViewModel,
    listItem: HomeViewModel.ListItem,
    isLastItem: Boolean,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { viewModel.onItemClicked(listItem) },
    ) {
        Text(listItem.name, modifier = Modifier.padding(16.dp))
        if (!isLastItem)
            Divider(color = Color.Black, thickness = 1.dp)
    }
}