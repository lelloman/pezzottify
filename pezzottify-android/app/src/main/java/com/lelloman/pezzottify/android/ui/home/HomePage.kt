package com.lelloman.pezzottify.android.ui.home

import androidx.compose.foundation.layout.Box
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.hilt.navigation.compose.hiltViewModel

@Composable
fun HomePage(viewModel: HomeViewModel = hiltViewModel()) {
    Box(contentAlignment = Alignment.Center) {
        Text("I'm home")
    }
}