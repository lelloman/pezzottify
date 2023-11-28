package com.lelloman.pezzottify.android.app.ui.player

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.pezzottify.android.app.R

@Composable
fun PlayerScreen(viewModel: PlayerViewModel = hiltViewModel()) {
    val state = viewModel.state.collectAsState()
    PlayerLayout(
        playerControlsState = state.value,
        onBackButtonClicked = viewModel::onBackButtonClicked,
        onPlayPauseButtonClicked = viewModel::onPlayPauseButtonClicked,
        onTrackPercentChanged = viewModel::onSeek,
        onSeekToNextButtonClicked = viewModel::onNextTrackButtonClicked,
        onSeekToPreviousButtonClicked = viewModel::onPreviousTrackButtonClicked,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Preview
@Composable
fun PlayerLayout(
    playerControlsState: PlayerViewModel.PlayerState = PlayerViewModel.PlayerState(
        albumName = "THE ALBUM NAME"
    ),
    onPlayPauseButtonClicked: () -> Unit = {},
    onBackButtonClicked: () -> Unit = {},
    onTrackPercentChanged: (Float) -> Unit = {},
    onSeekToNextButtonClicked: () -> Unit = {},
    onSeekToPreviousButtonClicked: () -> Unit = {},
) {
    Scaffold(
        topBar = {
            CenterAlignedTopAppBar(
                colors = TopAppBarDefaults.centerAlignedTopAppBarColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer,
                    titleContentColor = MaterialTheme.colorScheme.primary,
                ),
                title = {
                    Text(playerControlsState.albumName)
                },
                navigationIcon = {
                    IconButton(onClick = onBackButtonClicked) {
                        Icon(
                            imageVector = Icons.Filled.ArrowBack,
                            contentDescription = "Back"
                        )
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp)
                    .weight(1f),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    "Image placeholder",
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(color = Color(0x11000000)),
                    textAlign = TextAlign.Center,
                )
            }

            Slider(value = playerControlsState.trackPercent, onValueChange = onTrackPercentChanged)

            Row(
                modifier = Modifier
                    .fillMaxWidth(),
                horizontalArrangement = Arrangement.Center,
            ) {
                val buttonsModifier = Modifier
                    .size(96.dp)
                    .padding(16.dp)
                IconButton(modifier = buttonsModifier, onClick = onSeekToPreviousButtonClicked) {
                    Icon(
                        painter = painterResource(R.drawable.baseline_skip_previous_24),
                        contentDescription = "seek to previous",
                        modifier = Modifier.fillMaxSize()
                    )
                }
                Spacer(Modifier.width(48.dp))
                IconButton(modifier = buttonsModifier, onClick = onPlayPauseButtonClicked) {
                    Crossfade(
                        targetState = playerControlsState.isPlaying,
                        label = ""
                    ) { isPlaying ->
                        val res =
                            if (isPlaying) R.drawable.baseline_pause_24 else R.drawable.baseline_play_arrow_24
                        Icon(
                            painter = painterResource(res),
                            contentDescription = "play/pause",
                            modifier = Modifier.fillMaxSize()
                        )
                    }
                }
                Spacer(Modifier.width(48.dp))
                IconButton(modifier = buttonsModifier, onClick = onSeekToNextButtonClicked) {
                    Icon(
                        painter = painterResource(R.drawable.baseline_skip_next_24),
                        contentDescription = "seek to previous",
                        modifier = Modifier.fillMaxSize()
                    )
                }
            }
            Spacer(modifier = Modifier.height(32.dp))
        }
    }
}