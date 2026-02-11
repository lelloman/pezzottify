package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.domain.settings.BackgroundSyncInterval
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BackgroundSyncSection(
    selectedInterval: BackgroundSyncInterval,
    onIntervalChanged: (BackgroundSyncInterval) -> Unit,
    modifier: Modifier = Modifier,
) {
    var expanded by remember { mutableStateOf(false) }

    Column(modifier = modifier) {
        Text(
            text = stringResource(R.string.background_sync_section),
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = stringResource(R.string.background_sync_description),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(modifier = Modifier.height(16.dp))

        ExposedDropdownMenuBox(
            expanded = expanded,
            onExpandedChange = { expanded = it },
        ) {
            OutlinedTextField(
                value = intervalLabel(selectedInterval),
                onValueChange = {},
                readOnly = true,
                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
                modifier = Modifier
                    .menuAnchor(MenuAnchorType.PrimaryNotEditable)
                    .fillMaxWidth(),
            )

            ExposedDropdownMenu(
                expanded = expanded,
                onDismissRequest = { expanded = false },
            ) {
                BackgroundSyncInterval.entries.forEach { interval ->
                    DropdownMenuItem(
                        text = { Text(intervalLabel(interval)) },
                        onClick = {
                            onIntervalChanged(interval)
                            expanded = false
                        },
                        contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                    )
                }
            }
        }
    }
}

@Composable
private fun intervalLabel(interval: BackgroundSyncInterval): String = when (interval) {
    BackgroundSyncInterval.Minutes15 -> stringResource(R.string.sync_interval_15min)
    BackgroundSyncInterval.Minutes30 -> stringResource(R.string.sync_interval_30min)
    BackgroundSyncInterval.Hours1 -> stringResource(R.string.sync_interval_1h)
    BackgroundSyncInterval.Hours2 -> stringResource(R.string.sync_interval_2h)
    BackgroundSyncInterval.Hours4 -> stringResource(R.string.sync_interval_4h)
    BackgroundSyncInterval.Hours6 -> stringResource(R.string.sync_interval_6h)
    BackgroundSyncInterval.Hours12 -> stringResource(R.string.sync_interval_12h)
    BackgroundSyncInterval.Hours24 -> stringResource(R.string.sync_interval_24h)
    BackgroundSyncInterval.Disabled -> stringResource(R.string.sync_interval_disabled)
}

@Preview(showBackground = true)
@Composable
private fun BackgroundSyncSectionPreview() {
    PezzottifyTheme {
        BackgroundSyncSection(
            selectedInterval = BackgroundSyncInterval.Hours12,
            onIntervalChanged = {},
            modifier = Modifier.padding(16.dp),
        )
    }
}
