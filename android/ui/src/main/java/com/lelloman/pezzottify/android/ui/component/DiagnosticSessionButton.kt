package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lelloman.androidoscopy.Androidoscopy
import com.lelloman.androidoscopy.ui.SessionActivity
import com.lelloman.pezzottify.android.ui.R

/** Navigation, not a persisted setting: only the SDK owns session activation/expiry. */
@Composable
fun DiagnosticSessionButton(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val session by Androidoscopy.sessionState.collectAsStateWithLifecycle()
    Column(modifier) {
        OutlinedButton(onClick = { SessionActivity.launch(context) }) {
            Text(
                stringResource(
                    if (session.active) R.string.manage_diagnostic_session else R.string.diagnostic_session,
                ),
            )
        }
        Text(
            stringResource(R.string.diagnostic_session_description),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
