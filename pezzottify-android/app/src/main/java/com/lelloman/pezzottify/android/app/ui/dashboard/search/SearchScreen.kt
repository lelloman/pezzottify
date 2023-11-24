package com.lelloman.pezzottify.android.app.ui.dashboard.search

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.hilt.navigation.compose.hiltViewModel

@Composable
fun SearchScreen(viewModel: SearchViewModel = hiltViewModel()) {
    Box(modifier = Modifier.fillMaxSize()) {
        Text("In search of what")
    }
}