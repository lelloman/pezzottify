package com.lelloman.pezzottify.android.ui.screen.main.devices

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Laptop
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.SkipPrevious
import androidx.compose.material.icons.filled.Smartphone
import androidx.compose.material.icons.filled.Tv
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DevicesScreen(
    navController: NavController,
) {
    val viewModel = hiltViewModel<DevicesScreenViewModel>()
    val state by viewModel.state.collectAsState()
    val sharePolicy by viewModel.sharePolicy.collectAsState()
    var showPolicyEditor by remember { mutableStateOf(false) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Devices") },
                navigationIcon = {
                    IconButton(onClick = { navController.popBackStack() }) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { paddingValues ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item { Spacer(modifier = Modifier.height(4.dp)) }
            if (state.devices.isEmpty()) {
                item {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 24.dp),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Text(
                            text = "No devices connected",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            } else {
                items(state.devices, key = { it.id }) { device ->
                    val isControlling = state.remoteControlDeviceId == device.id
                    DeviceCard(
                        device = device,
                        isControlling = isControlling,
                        onPlayPause = {
                            val cmd = if (device.isPlaying) "pause" else "play"
                            viewModel.sendCommand(cmd, emptyMap(), device.id)
                        },
                        onSkipNext = {
                            viewModel.sendCommand("next", emptyMap(), device.id)
                        },
                        onSkipPrev = {
                            viewModel.sendCommand("prev", emptyMap(), device.id)
                        },
                        onSeek = { positionSec ->
                            viewModel.seekRemote(device.id, positionSec)
                        },
                        onControlDevice = {
                            viewModel.enterRemoteMode(device.id, device.name)
                        },
                        onDisconnectDevice = {
                            viewModel.exitRemoteMode()
                        },
                    )
                }
            }
            item {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
                    shape = RoundedCornerShape(16.dp),
                ) {
                    ListItem(
                        headlineContent = { Text("Device Sharing (This Device)") },
                        supportingContent = {
                            if (sharePolicy.isSaving) {
                                Text("Saving…")
                            } else {
                                Text(if (showPolicyEditor) "Tap to collapse" else "Tap to configure")
                            }
                        },
                        trailingContent = {
                            Icon(
                                imageVector = if (showPolicyEditor) {
                                    Icons.Filled.ExpandLess
                                } else {
                                    Icons.Filled.ExpandMore
                                },
                                contentDescription = null,
                            )
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .clickable { showPolicyEditor = !showPolicyEditor }
                            .padding(horizontal = 4.dp),
                    )

                    if (showPolicyEditor) {
                        SharePolicyCard(
                            state = sharePolicy,
                            onModeChange = viewModel::updatePolicyMode,
                            onAllowUsersChange = viewModel::updateAllowUsers,
                            onDenyUsersChange = viewModel::updateDenyUsers,
                            onAllowAdminChange = viewModel::updateAllowAdmin,
                            onAllowRegularChange = viewModel::updateAllowRegular,
                            onSave = viewModel::saveSharePolicy,
                        )
                    }
                }
            }
            item { Spacer(modifier = Modifier.height(4.dp)) }
        }
    }
}

@Composable
private fun SharePolicyCard(
    state: DeviceSharePolicyUiState,
    onModeChange: (String) -> Unit,
    onAllowUsersChange: (String) -> Unit,
    onDenyUsersChange: (String) -> Unit,
    onAllowAdminChange: (Boolean) -> Unit,
    onAllowRegularChange: (Boolean) -> Unit,
    onSave: () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = RoundedCornerShape(12.dp),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (state.isLoading) {
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            }
            if (state.error != null) {
                Text(
                    text = state.error,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                RadioButton(
                    selected = state.mode == "deny_everyone",
                    onClick = { onModeChange("deny_everyone") },
                )
                Text("Deny everyone")
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                RadioButton(
                    selected = state.mode == "allow_everyone",
                    onClick = { onModeChange("allow_everyone") },
                )
                Text("Allow everyone")
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                RadioButton(
                    selected = state.mode == "custom",
                    onClick = { onModeChange("custom") },
                )
                Text("Custom")
            }

            if (state.mode == "custom") {
                OutlinedTextField(
                    value = state.allowUsers,
                    onValueChange = onAllowUsersChange,
                    label = { Text("Allow users (IDs)") },
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = state.denyUsers,
                    onValueChange = onDenyUsersChange,
                    label = { Text("Deny users (IDs)") },
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(
                        checked = state.allowAdmin,
                        onCheckedChange = onAllowAdminChange,
                    )
                    Text("Allow Admin")
                    Spacer(modifier = Modifier.width(12.dp))
                    Checkbox(
                        checked = state.allowRegular,
                        onCheckedChange = onAllowRegularChange,
                    )
                    Text("Allow Regular")
                }
            }

            Button(
                onClick = onSave,
                enabled = !state.isSaving,
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
                modifier = Modifier.align(Alignment.End),
            ) {
                Text(if (state.isSaving) "Saving…" else "Save Policy")
            }
        }
    }
}

@Composable
private fun DeviceCard(
    device: DeviceUiState,
    isControlling: Boolean,
    onPlayPause: () -> Unit,
    onSkipNext: () -> Unit,
    onSkipPrev: () -> Unit,
    onSeek: (Double) -> Unit,
    onControlDevice: () -> Unit,
    onDisconnectDevice: () -> Unit,
) {
    val borderColor = if (device.isThisDevice) {
        MaterialTheme.colorScheme.primary
    } else if (isControlling) {
        MaterialTheme.colorScheme.tertiary
    } else {
        MaterialTheme.colorScheme.outlineVariant
    }

    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer,
        ),
        border = CardDefaults.outlinedCardBorder().copy(
            width = if (device.isThisDevice || isControlling) 2.dp else 1.dp,
            brush = androidx.compose.ui.graphics.SolidColor(borderColor),
        ),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // Device header
            Row(
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = when (device.deviceType) {
                        "web" -> Icons.Filled.Laptop
                        "android_tv" -> Icons.Filled.Tv
                        else -> Icons.Filled.Smartphone
                    },
                    contentDescription = null,
                    tint = if (device.isThisDevice) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    modifier = Modifier.size(20.dp),
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = device.name,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                if (device.isThisDevice) {
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "this device",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
            }
            val typeLabel = deviceTypeLabel(device.deviceType)
            if (typeLabel != null) {
                Text(
                    text = typeLabel,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .clip(RoundedCornerShape(8.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant)
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                )
            }

            // Remote mode connect/disconnect should be available even when target is idle.
            if (!device.isThisDevice) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                ) {
                    if (isControlling) {
                        OutlinedButton(
                            onClick = onDisconnectDevice,
                            colors = ButtonDefaults.outlinedButtonColors(
                                contentColor = MaterialTheme.colorScheme.error,
                            ),
                        ) {
                            Text("Disconnect")
                        }
                    } else {
                        Button(
                            onClick = onControlDevice,
                        ) {
                            Text("Control this device")
                        }
                    }
                }
            }

            // Playback info
            if (device.trackTitle != null) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    NullablePezzottifyImage(
                        url = device.albumImageUrl,
                        shape = PezzottifyImageShape.MiniPlayer,
                        modifier = Modifier.clip(RoundedCornerShape(6.dp)),
                    )
                    Spacer(modifier = Modifier.width(12.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = device.trackTitle,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurface,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        if (device.artistName != null) {
                            Text(
                                text = device.artistName,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }

                // Controls for remote devices
                if (!device.isThisDevice) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.Center,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        IconButton(onClick = onSkipPrev) {
                            Icon(
                                Icons.Filled.SkipPrevious,
                                contentDescription = "Previous",
                                modifier = Modifier.size(28.dp),
                            )
                        }
                        IconButton(onClick = onPlayPause) {
                            Icon(
                                if (device.isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow,
                                contentDescription = if (device.isPlaying) "Pause" else "Play",
                                modifier = Modifier.size(36.dp),
                            )
                        }
                        IconButton(onClick = onSkipNext) {
                            Icon(
                                Icons.Filled.SkipNext,
                                contentDescription = "Next",
                                modifier = Modifier.size(28.dp),
                            )
                        }
                    }
                }

                // Progress bar / seek bar
                if (device.durationMs > 0) {
                    val durationSec = device.durationMs / 1000.0
                    if (!device.isThisDevice) {
                        // Seekable slider for remote devices
                        var isDragging by remember { mutableStateOf(false) }
                        var sliderPosition by remember {
                            mutableFloatStateOf(
                                (device.positionSec / durationSec).toFloat().coerceIn(0f, 1f)
                            )
                        }
                        // Update from interpolated position only when not dragging
                        LaunchedEffect(device.positionSec) {
                            if (!isDragging) {
                                sliderPosition = (device.positionSec / durationSec).toFloat().coerceIn(0f, 1f)
                            }
                        }
                        Column {
                            Slider(
                                value = sliderPosition,
                                onValueChange = {
                                    isDragging = true
                                    sliderPosition = it
                                },
                                onValueChangeFinished = {
                                    isDragging = false
                                    onSeek(sliderPosition * durationSec)
                                },
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(24.dp),
                            )
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                Text(
                                    text = formatSeconds((sliderPosition * durationSec).toInt()),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                                Text(
                                    text = formatSeconds(durationSec.toInt()),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    } else {
                        // Read-only progress for this device
                        val progress = (device.positionSec * 1000.0 / device.durationMs)
                            .toFloat()
                            .coerceIn(0f, 1f)
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            LinearProgressIndicator(
                                progress = { progress },
                                modifier = Modifier
                                    .weight(1f)
                                    .height(4.dp)
                                    .clip(RoundedCornerShape(2.dp)),
                                color = MaterialTheme.colorScheme.primary,
                                trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = "${formatSeconds(device.positionSec.toInt())} / ${formatSeconds(durationSec.toInt())}",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            } else {
                Text(
                    text = "Not playing",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontStyle = androidx.compose.ui.text.font.FontStyle.Italic,
                )
            }
        }
    }
}

private fun deviceTypeLabel(deviceType: String): String? = when (deviceType) {
    "web" -> "Web"
    "android_tv" -> "TV"
    "android" -> "Phone"
    else -> null
}

private fun formatSeconds(totalSeconds: Int): String {
    val hours = totalSeconds / 3600
    val minutes = (totalSeconds % 3600) / 60
    val seconds = totalSeconds % 60
    return if (hours > 0) {
        "%d:%02d:%02d".format(hours, minutes, seconds)
    } else {
        "%d:%02d".format(minutes, seconds)
    }
}
