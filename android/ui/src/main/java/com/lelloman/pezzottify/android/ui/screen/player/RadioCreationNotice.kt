package com.lelloman.pezzottify.android.ui.screen.player

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R

enum class RadioCreationStatusUi { Idle, Creating, Error, TimedOut, Empty }

@Composable
fun RadioCreationNotice(status: RadioCreationStatusUi, onRetry: () -> Unit, onDismiss: () -> Unit) {
    if (status == RadioCreationStatusUi.Idle) return
    Surface(color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Row(
            modifier = Modifier.fillMaxWidth().semantics { liveRegion = LiveRegionMode.Polite }.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (status == RadioCreationStatusUi.Creating) CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
            Text(stringResource(when (status) {
                RadioCreationStatusUi.Creating -> R.string.radio_creating
                RadioCreationStatusUi.TimedOut -> R.string.radio_creation_timeout
                RadioCreationStatusUi.Empty -> R.string.radio_creation_empty
                else -> R.string.radio_creation_error
            }), modifier = Modifier.weight(1f))
            if (status != RadioCreationStatusUi.Creating) TextButton(onClick = onRetry) { Text(stringResource(R.string.radio_creation_retry)) }
            TextButton(onClick = onDismiss) { Text(stringResource(if (status == RadioCreationStatusUi.Creating) R.string.radio_creation_cancel else R.string.radio_creation_dismiss)) }
        }
    }
}
