package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun SmartSearchSection(
    isEnabled: Boolean,
    onEnabledChanged: (Boolean) -> Unit,
    isExcludeUnavailableEnabled: Boolean,
    onExcludeUnavailableChanged: (Boolean) -> Unit,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier) {
        Text(
            text = stringResource(R.string.search_section),
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )

        Spacer(modifier = Modifier.height(16.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = stringResource(R.string.smart_search),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    text = stringResource(R.string.smart_search_description),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Switch(
                checked = isEnabled,
                onCheckedChange = onEnabledChanged
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = stringResource(R.string.exclude_unavailable),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    text = stringResource(R.string.exclude_unavailable_description),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Switch(
                checked = isExcludeUnavailableEnabled,
                onCheckedChange = onExcludeUnavailableChanged
            )
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun SmartSearchSectionPreviewEnabled() {
    PezzottifyTheme {
        SmartSearchSection(
            isEnabled = true,
            onEnabledChanged = {},
            isExcludeUnavailableEnabled = true,
            onExcludeUnavailableChanged = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun SmartSearchSectionPreviewDisabled() {
    PezzottifyTheme {
        SmartSearchSection(
            isEnabled = false,
            onEnabledChanged = {},
            isExcludeUnavailableEnabled = false,
            onExcludeUnavailableChanged = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}
