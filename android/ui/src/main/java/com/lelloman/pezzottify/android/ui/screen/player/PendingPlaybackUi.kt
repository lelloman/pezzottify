package com.lelloman.pezzottify.android.ui.screen.player

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R

/** Playback that has been requested but is not playable yet (e.g. a radio being created). */
data class PendingPlaybackUi(
    val label: String,
    val status: Status,
) {
    enum class Status { Creating, Error, TimedOut, Empty }

    val isCreating: Boolean get() = status == Status.Creating
}

/** "Creating radio · <label>" while creating, the error message otherwise. */
@Composable
fun pendingPlaybackText(pending: PendingPlaybackUi): String = when (pending.status) {
    PendingPlaybackUi.Status.Creating ->
        if (pending.label.isBlank()) stringResource(R.string.radio_creating)
        else stringResource(R.string.radio_creating_with_label, pending.label)
    PendingPlaybackUi.Status.TimedOut -> stringResource(R.string.radio_creation_timeout)
    PendingPlaybackUi.Status.Empty -> stringResource(R.string.radio_creation_empty)
    PendingPlaybackUi.Status.Error -> stringResource(R.string.radio_creation_error)
}

/** Full-screen presentation of a pending playback, used by the player screen. */
@Composable
fun PendingPlaybackContent(
    pending: PendingPlaybackUi,
    onCancel: () -> Unit,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(24.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (pending.isCreating) {
            CircularProgressIndicator()
            Spacer(modifier = Modifier.height(24.dp))
        } else if (pending.label.isNotBlank()) {
            Text(
                text = pending.label,
                style = MaterialTheme.typography.titleMedium,
                textAlign = TextAlign.Center,
            )
            Spacer(modifier = Modifier.height(8.dp))
        }
        Text(
            text = pendingPlaybackText(pending),
            style = MaterialTheme.typography.bodyLarge,
            textAlign = TextAlign.Center,
            color = if (pending.isCreating) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.error,
        )
        Spacer(modifier = Modifier.height(24.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (pending.isCreating) {
                OutlinedButton(onClick = onCancel) { Text(stringResource(R.string.radio_creation_cancel)) }
            } else {
                OutlinedButton(onClick = onCancel) { Text(stringResource(R.string.radio_creation_dismiss)) }
                OutlinedButton(onClick = onRetry) { Text(stringResource(R.string.radio_creation_retry)) }
            }
        }
    }
}

/** Quiet inline hint shown when the radio could not be extended with more tracks. */
@Composable
fun RadioContinuationErrorHint(
    onRetry: () -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier.semantics { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.radio_continuation_error),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        TextButton(onClick = onRetry) {
            Text(stringResource(R.string.radio_creation_retry), style = MaterialTheme.typography.labelSmall)
        }
        TextButton(onClick = onDismiss) {
            Text(stringResource(R.string.radio_creation_dismiss), style = MaterialTheme.typography.labelSmall)
        }
    }
}

