package com.lelloman.pezzottify.android.ui.screen.player

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import kotlinx.coroutines.flow.collectLatest
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.LoadingScreen
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.component.ScrollingArtistsRow
import com.lelloman.pezzottify.android.ui.component.ScrollingTextRow
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toArtist
import com.lelloman.pezzottify.android.ui.toQueue
import com.lelloman.pezzottify.android.ui.toTrack
import kotlinx.coroutines.launch
import kotlin.math.abs

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PlayerScreen(navController: NavController) {
    val viewModel = hiltViewModel<PlayerScreenViewModel>()
    val state by viewModel.state.collectAsState()
    val snackbarHostState = remember { SnackbarHostState() }

    // Collect toast events from the ViewModel
    LaunchedEffect(viewModel) {
        viewModel.toastEvents.collectLatest { message ->
            snackbarHostState.showSnackbar(message)
        }
    }

    PlayerScreenContent(
        state = state,
        actions = viewModel,
        navController = navController,
        snackbarHostState = snackbarHostState,
    )
}

private const val DISMISS_THRESHOLD = 0.3f

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PlayerScreenContent(
    state: PlayerScreenState,
    actions: PlayerScreenActions,
    navController: NavController,
    snackbarHostState: SnackbarHostState,
) {
    var dismissOffsetY by remember { mutableFloatStateOf(0f) }
    var isDraggingToDismiss by remember { mutableStateOf(false) }

    val scope = rememberCoroutineScope()
    val density = LocalDensity.current
    val configuration = LocalConfiguration.current
    val screenHeightPx = with(density) { configuration.screenHeightDp.dp.toPx() }

    val dismissOffsetAnimatable = remember { Animatable(0f) }

    val backgroundAlpha = if (isDraggingToDismiss || dismissOffsetY > 0f) {
        (1f - dismissOffsetY / screenHeightPx).coerceIn(0f, 1f)
    } else {
        1f
    }

    fun dismiss() {
        navController.popBackStack()
    }

    fun animateSnapBack() {
        scope.launch {
            dismissOffsetAnimatable.snapTo(dismissOffsetY)
            dismissOffsetAnimatable.animateTo(
                0f,
                animationSpec = spring(stiffness = Spring.StiffnessMedium)
            ) {
                dismissOffsetY = value
            }
        }
    }

    BackHandler {
        dismiss()
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.scrim.copy(alpha = backgroundAlpha * 0.32f))
            .pointerInput(Unit) {
                detectVerticalDragGestures(
                    onDragStart = { isDraggingToDismiss = true },
                    onDragEnd = {
                        isDraggingToDismiss = false
                        val dismissThresholdPx = screenHeightPx * DISMISS_THRESHOLD
                        if (dismissOffsetY > dismissThresholdPx) {
                            dismiss()
                        } else {
                            animateSnapBack()
                        }
                    },
                    onDragCancel = {
                        isDraggingToDismiss = false
                        animateSnapBack()
                    },
                    onVerticalDrag = { _, dragAmount ->
                        // Only allow dragging down (positive direction)
                        val newOffset = dismissOffsetY + dragAmount
                        dismissOffsetY = newOffset.coerceAtLeast(0f)
                    }
                )
            }
    ) {
        Scaffold(
            modifier = Modifier
                .graphicsLayer {
                    translationY = dismissOffsetY
                    alpha = backgroundAlpha
                },
            snackbarHost = { SnackbarHost(hostState = snackbarHostState) },
            topBar = {
                TopAppBar(
                    title = {
                        Text(
                            text = state.albumName,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.clickable {
                                if (state.albumId.isNotEmpty()) {
                                    navController.toAlbum(state.albumId)
                                }
                            }
                        )
                    },
                    navigationIcon = {
                        IconButton(onClick = { dismiss() }) {
                            Icon(
                                imageVector = Icons.AutoMirrored.Filled.KeyboardArrowLeft,
                                contentDescription = stringResource(R.string.back),
                                modifier = Modifier.size(32.dp)
                            )
                        }
                    },
                    actions = {
                        IconButton(onClick = { navController.toQueue() }) {
                            Icon(
                                painter = painterResource(R.drawable.baseline_queue_music_24),
                                contentDescription = stringResource(R.string.queue),
                                modifier = Modifier.size(24.dp)
                            )
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surface,
                    )
                )
            }
        ) { innerPadding ->
        if (state.isLoading) {
            LoadingScreen()
        } else {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .padding(horizontal = 24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Spacer(modifier = Modifier.height(16.dp))

                // Album art
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .aspectRatio(1f)
                        .clip(RoundedCornerShape(8.dp))
                        .clickable {
                            if (state.albumId.isNotEmpty()) {
                                navController.toAlbum(state.albumId)
                            }
                        }
                ) {
                    NullablePezzottifyImage(
                        url = state.albumImageUrl,
                        placeholder = PezzottifyImagePlaceholder.GenericImage,
                        shape = PezzottifyImageShape.FullSize,
                    )
                }

                Spacer(modifier = Modifier.height(24.dp))

                // Track info
                ScrollingTextRow(
                    text = state.trackName,
                    textStyle = MaterialTheme.typography.headlineSmall,
                    textColor = MaterialTheme.colorScheme.onSurface,
                    onClick = if (state.trackId.isNotEmpty()) {
                        { navController.toTrack(state.trackId) }
                    } else null
                )

                Spacer(modifier = Modifier.height(4.dp))

                ScrollingArtistsRow(
                    artists = state.artists,
                    textStyle = MaterialTheme.typography.bodyLarge,
                    textColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    onArtistClick = { artistId -> navController.toArtist(artistId) }
                )

                Spacer(modifier = Modifier.height(24.dp))

                // Progress bar
                ProgressSection(
                    progressPercent = state.trackProgressPercent,
                    progressSec = state.trackProgressSec,
                    durationSec = state.trackDurationSec,
                    onSeek = actions::seekToPercent,
                )

                Spacer(modifier = Modifier.height(16.dp))

                // Playback controls
                PlaybackControls(
                    isPlaying = state.isPlaying,
                    hasNext = state.hasNextTrack,
                    hasPrevious = state.hasPreviousTrack,
                    shuffleEnabled = state.shuffleEnabled,
                    repeatMode = state.repeatMode,
                    onPlayPause = actions::clickOnPlayPause,
                    onSkipNext = actions::clickOnSkipNext,
                    onSkipPrevious = actions::clickOnSkipPrevious,
                    onShuffle = actions::clickOnShuffle,
                    onRepeat = actions::clickOnRepeat,
                )

                Spacer(modifier = Modifier.height(24.dp))

                Spacer(modifier = Modifier.weight(1f))

                // Volume control
                VolumeControl(
                    volume = state.volume,
                    isMuted = state.isMuted,
                    onVolumeChange = actions::setVolume,
                    onToggleMute = actions::toggleMute,
                )

                Spacer(modifier = Modifier.height(24.dp))
            }
        }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ProgressSection(
    progressPercent: Float,
    progressSec: Int,
    durationSec: Int,
    onSeek: (Float) -> Unit,
) {
    var isDragging by remember { mutableFloatStateOf(-1f) }
    var seekTarget by remember { mutableFloatStateOf(-1f) }

    // Reset seek target once player state catches up (within 2%)
    if (seekTarget >= 0f && abs(progressPercent - seekTarget) < 2f) {
        seekTarget = -1f
    }

    val displayPercent = when {
        isDragging >= 0f -> isDragging
        seekTarget >= 0f -> seekTarget
        else -> progressPercent
    }

    Column(modifier = Modifier.fillMaxWidth()) {
        Slider(
            value = displayPercent / 100f,
            onValueChange = { isDragging = it * 100f },
            onValueChangeFinished = {
                if (isDragging >= 0f) {
                    onSeek(isDragging)
                    seekTarget = isDragging
                    isDragging = -1f
                }
            },
            colors = SliderDefaults.colors(
                thumbColor = MaterialTheme.colorScheme.primary,
                activeTrackColor = MaterialTheme.colorScheme.primary,
                inactiveTrackColor = MaterialTheme.colorScheme.surfaceVariant,
            ),
            thumb = {
                Box(
                    modifier = Modifier
                        .size(width = 12.dp, height = 20.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Box(
                        modifier = Modifier
                            .size(12.dp)
                            .background(MaterialTheme.colorScheme.primary, CircleShape)
                    )
                }
            },
            track = { sliderState ->
                val fraction = (sliderState.value - sliderState.valueRange.start) /
                    (sliderState.valueRange.endInclusive - sliderState.valueRange.start)
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(20.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(4.dp)
                            .clip(RoundedCornerShape(2.dp))
                            .background(MaterialTheme.colorScheme.surfaceVariant)
                    ) {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth(fraction)
                                .height(4.dp)
                                .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(2.dp))
                        )
                    }
                }
            }
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = formatTime(progressSec),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = formatTime(durationSec),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun PlaybackControls(
    isPlaying: Boolean,
    hasNext: Boolean,
    hasPrevious: Boolean,
    shuffleEnabled: Boolean,
    repeatMode: RepeatModeUi,
    onPlayPause: () -> Unit,
    onSkipNext: () -> Unit,
    onSkipPrevious: () -> Unit,
    onShuffle: () -> Unit,
    onRepeat: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Shuffle
        IconButton(
            onClick = onShuffle,
            modifier = Modifier.size(48.dp),
        ) {
            Icon(
                painter = painterResource(R.drawable.baseline_shuffle_24),
                contentDescription = stringResource(R.string.shuffle),
                modifier = Modifier.size(24.dp),
                tint = if (shuffleEnabled) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.onSurface.copy(alpha = 0.6f)
                }
            )
        }

        // Skip previous
        IconButton(
            onClick = onSkipPrevious,
            enabled = hasPrevious,
            modifier = Modifier.size(56.dp),
        ) {
            Icon(
                painter = painterResource(R.drawable.baseline_skip_previous_24),
                contentDescription = stringResource(R.string.previous),
                modifier = Modifier.size(36.dp),
                tint = if (hasPrevious) {
                    MaterialTheme.colorScheme.onSurface
                } else {
                    MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                }
            )
        }

        Spacer(modifier = Modifier.width(8.dp))

        // Play/Pause
        IconButton(
            onClick = onPlayPause,
            modifier = Modifier
                .size(72.dp)
                .background(MaterialTheme.colorScheme.primary, CircleShape),
        ) {
            Icon(
                painter = painterResource(
                    if (isPlaying) R.drawable.baseline_pause_24 else R.drawable.baseline_play_arrow_24
                ),
                contentDescription = if (isPlaying) stringResource(R.string.pause) else stringResource(R.string.play),
                modifier = Modifier.size(40.dp),
                tint = MaterialTheme.colorScheme.onSurface,
            )
        }

        Spacer(modifier = Modifier.width(8.dp))

        // Skip next
        IconButton(
            onClick = onSkipNext,
            enabled = hasNext,
            modifier = Modifier.size(56.dp),
        ) {
            Icon(
                painter = painterResource(R.drawable.baseline_skip_next_24),
                contentDescription = stringResource(R.string.next),
                modifier = Modifier.size(36.dp),
                tint = if (hasNext) {
                    MaterialTheme.colorScheme.onSurface
                } else {
                    MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                }
            )
        }

        // Repeat
        IconButton(
            onClick = onRepeat,
            modifier = Modifier.size(48.dp),
        ) {
            Icon(
                painter = painterResource(
                    when (repeatMode) {
                        RepeatModeUi.OFF -> R.drawable.baseline_repeat_24
                        RepeatModeUi.ALL -> R.drawable.baseline_repeat_24
                        RepeatModeUi.ONE -> R.drawable.baseline_repeat_one_24
                    }
                ),
                contentDescription = stringResource(R.string.repeat),
                modifier = Modifier.size(24.dp),
                tint = when (repeatMode) {
                    RepeatModeUi.OFF -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.6f)
                    RepeatModeUi.ALL, RepeatModeUi.ONE -> MaterialTheme.colorScheme.primary
                }
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun VolumeControl(
    volume: Float,
    isMuted: Boolean,
    onVolumeChange: (Float) -> Unit,
    onToggleMute: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onToggleMute) {
            Icon(
                painter = painterResource(R.drawable.baseline_volume_off_24),
                contentDescription = if (isMuted) stringResource(R.string.unmute) else stringResource(R.string.mute),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        Slider(
            value = if (isMuted) 0f else volume,
            onValueChange = {
                if (isMuted) onToggleMute()
                onVolumeChange(it)
            },
            modifier = Modifier.weight(1f),
            colors = SliderDefaults.colors(
                thumbColor = MaterialTheme.colorScheme.primary,
                activeTrackColor = MaterialTheme.colorScheme.primary,
                inactiveTrackColor = MaterialTheme.colorScheme.surfaceVariant,
            ),
            thumb = {
                Box(
                    modifier = Modifier
                        .size(width = 12.dp, height = 20.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Box(
                        modifier = Modifier
                            .size(12.dp)
                            .background(MaterialTheme.colorScheme.primary, CircleShape)
                    )
                }
            },
            track = { sliderState ->
                val fraction = (sliderState.value - sliderState.valueRange.start) /
                    (sliderState.valueRange.endInclusive - sliderState.valueRange.start)
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(20.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(4.dp)
                            .clip(RoundedCornerShape(2.dp))
                            .background(MaterialTheme.colorScheme.surfaceVariant)
                    ) {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth(fraction)
                                .height(4.dp)
                                .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(2.dp))
                        )
                    }
                }
            }
        )

        IconButton(onClick = {
            if (isMuted) onToggleMute()
            onVolumeChange(1f)
        }) {
            Icon(
                painter = painterResource(R.drawable.baseline_volume_up_24),
                contentDescription = stringResource(R.string.max_volume),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

private fun formatTime(seconds: Int): String {
    val mins = seconds / 60
    val secs = seconds % 60
    return "%d:%02d".format(mins, secs)
}
