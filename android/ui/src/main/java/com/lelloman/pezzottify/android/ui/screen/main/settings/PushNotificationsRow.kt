package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.domain.push.PushDistributor
import com.lelloman.pezzottify.android.domain.push.PushState
import com.lelloman.pezzottify.android.domain.push.PushStatus
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

/** The UnifiedPush entry of the Notifications section. */
@Composable
fun PushNotificationsRow(
    state: PushState,
    onEnabledChanged: (Boolean) -> Unit,
    onChooseDistributor: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    var showPicker by remember { mutableStateOf(false) }
    Column(modifier = modifier) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = stringResource(R.string.push_notifications),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    text = pushStatusText(state),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Switch(
                checked = state.enabled,
                onCheckedChange = onEnabledChanged,
                enabled = state.status != PushStatus.LoggedOut,
            )
        }
        if (state.enabled && state.distributors.size > 1) {
            TextButton(onClick = { showPicker = true }) {
                Text(stringResource(R.string.push_choose_app))
            }
        }
    }
    if (showPicker) {
        PushDistributorPicker(
            distributors = state.distributors,
            selected = state.status.distributorPackage(),
            onPicked = {
                showPicker = false
                onChooseDistributor(it)
            },
            onDismiss = { showPicker = false },
        )
    }
}

@Composable
internal fun pushStatusText(state: PushState): String = when (val status = state.status) {
    PushStatus.LoggedOut -> stringResource(R.string.push_status_logged_out)
    PushStatus.Disabled -> stringResource(R.string.push_status_off)
    PushStatus.NoDistributor -> stringResource(R.string.push_status_no_distributor)
    PushStatus.ChooseDistributor -> stringResource(R.string.push_status_choose)
    PushStatus.ServerUnsupported -> stringResource(R.string.push_status_server_unsupported)
    is PushStatus.Registering -> stringResource(R.string.push_status_registering, status.distributor.label)
    is PushStatus.Registered -> stringResource(R.string.push_status_registered, status.distributor.label)
    is PushStatus.Failed -> status.distributor
        ?.let { stringResource(R.string.push_status_failed_with, it.label) }
        ?: stringResource(R.string.push_status_failed)
}

internal fun PushStatus.distributorPackage(): String? = when (this) {
    is PushStatus.Registered -> distributor.packageName
    is PushStatus.Registering -> distributor.packageName
    is PushStatus.Failed -> distributor?.packageName
    else -> null
}

@Composable
private fun PushDistributorPicker(
    distributors: List<PushDistributor>,
    selected: String?,
    onPicked: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.push_choose_app)) },
        text = {
            Column {
                distributors.forEach { distributor ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onPicked(distributor.packageName) }
                            .padding(vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(
                            selected = distributor.packageName == selected,
                            onClick = { onPicked(distributor.packageName) },
                        )
                        Text(distributor.label, style = MaterialTheme.typography.bodyLarge)
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(android.R.string.cancel)) }
        },
    )
}

@Preview(showBackground = true)
@Composable
private fun PushNotificationsRowPreview() {
    PezzottifyTheme {
        val ntfy = PushDistributor("io.heckel.ntfy", "ntfy")
        PushNotificationsRow(
            state = PushState(
                enabled = true,
                status = PushStatus.Registered(ntfy),
                distributors = listOf(ntfy, PushDistributor("org.unifiedpush.distributor.nextpush", "NextPush")),
            ),
            onEnabledChanged = {},
            onChooseDistributor = {},
        )
    }
}
