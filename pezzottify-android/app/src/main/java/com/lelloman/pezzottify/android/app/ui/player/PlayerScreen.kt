package com.lelloman.pezzottify.android.app.ui.player

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import coil.Coil
import coil.compose.AsyncImage
import coil.imageLoader
import com.lelloman.pezzottify.android.app.R

interface PlayerScreenController {
    fun onBackButtonClicked()
    fun onPlayPauseButtonClicked()
    fun onSeek(to: Float)
    fun onNextTrackButtonClicked()
    fun onPreviousTrackButtonClicked()
}

class ViewModelController(private val viewModel: PlayerViewModel) : PlayerScreenController {
    override fun onBackButtonClicked() = viewModel.onBackButtonClicked()
    override fun onPlayPauseButtonClicked() = viewModel.onPlayPauseButtonClicked()
    override fun onSeek(to: Float) = viewModel.onSeek(to)
    override fun onNextTrackButtonClicked() = viewModel.onNextTrackButtonClicked()
    override fun onPreviousTrackButtonClicked() = viewModel.onPreviousTrackButtonClicked()
}

class StubPlayerScreenController : PlayerScreenController {
    override fun onBackButtonClicked() = Unit
    override fun onPlayPauseButtonClicked() = Unit
    override fun onSeek(to: Float) = Unit
    override fun onNextTrackButtonClicked() = Unit
    override fun onPreviousTrackButtonClicked() = Unit
}

@Composable
fun PlayerScreen(viewModel: PlayerViewModel = hiltViewModel()) {
    val state = viewModel.state.collectAsState()
    PlayerLayout(
        playerControlsState = state.value,
        controller = ViewModelController(viewModel),
    )
}

@OptIn(
    ExperimentalMaterial3Api::class,
    ExperimentalFoundationApi::class,
)
@Preview
@Composable
fun PlayerLayout(
    playerControlsState: PlayerViewModel.PlayerState = PlayerViewModel.PlayerState(
        albumName = "THE ALBUM NAME",
        trackName = "A very very very very very very loooooong track name that cant possibly fit on the screen",
        trackImageUrl = "https://pics.craiyon.com/2023-05-25/a1c8db716f044ba983bc20b6ab5bab91.webp",
    ),
    controller: PlayerScreenController = StubPlayerScreenController(),
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
                    IconButton(onClick = controller::onBackButtonClicked) {
                        Icon(
                            imageVector = Icons.Filled.ArrowBack, contentDescription = "Back"
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
                AsyncImage(
                    model = playerControlsState.trackImageUrl,
                    contentDescription = null,
                    modifier = Modifier.fillMaxWidth()
                        .aspectRatio(1f, true)
                )
            }

            Text(
                playerControlsState.trackName,
                modifier = Modifier
                    .padding(32.dp)
                    .basicMarquee(velocity = 60.dp)
                    .safeDrawingPadding(),
            )
            Slider(
                modifier = Modifier.padding(horizontal = 16.dp),
                value = playerControlsState.trackPercent,
                onValueChange = controller::onSeek,
            )

            PlayerControlsRow(playerControlsState, controller)
            Spacer(modifier = Modifier.height(32.dp))
        }
    }
}

@Composable
fun PlayerControlsRow(
    state: PlayerViewModel.PlayerState, controller: PlayerScreenController
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.Center,
    ) {
        val buttonsModifier = Modifier
            .size(96.dp)
            .padding(16.dp)
        IconButton(modifier = buttonsModifier, onClick = controller::onPreviousTrackButtonClicked) {
            Icon(
                painter = painterResource(R.drawable.baseline_skip_previous_24),
                contentDescription = "seek to previous",
                modifier = Modifier.fillMaxSize()
            )
        }
        Spacer(Modifier.width(48.dp))
        IconButton(modifier = buttonsModifier, onClick = controller::onPlayPauseButtonClicked) {
            Crossfade(
                targetState = state.isPlaying, label = ""
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
        IconButton(modifier = buttonsModifier, onClick = controller::onNextTrackButtonClicked) {
            Icon(
                painter = painterResource(R.drawable.baseline_skip_next_24),
                contentDescription = "seek to previous",
                modifier = Modifier.fillMaxSize()
            )
        }
    }
}