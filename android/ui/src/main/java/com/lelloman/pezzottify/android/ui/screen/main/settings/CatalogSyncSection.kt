package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.MaterialTheme
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
fun CatalogSyncSection(
    isResyncing: Boolean,
    resyncResult: SkeletonResyncResult?,
    onForceResync: () -> Unit,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier) {
        Text(
            text = stringResource(R.string.catalog_sync),
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
                    text = stringResource(R.string.force_resync_catalog),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    text = stringResource(R.string.catalog_sync_description),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            if (isResyncing) {
                PezzottifyLoader(size = LoaderSize.Small)
            } else {
                Button(
                    onClick = onForceResync,
                    enabled = !isResyncing
                ) {
                    Text(text = stringResource(R.string.catalog_sync_button))
                }
            }
        }

        // Show result status
        resyncResult?.let { result ->
            Spacer(modifier = Modifier.height(8.dp))
            val (text, color) = when (result) {
                is SkeletonResyncResult.Success -> stringResource(R.string.catalog_sync_success) to MaterialTheme.colorScheme.primary
                is SkeletonResyncResult.AlreadyUpToDate -> stringResource(R.string.catalog_sync_up_to_date) to MaterialTheme.colorScheme.tertiary
                is SkeletonResyncResult.Failed -> stringResource(R.string.catalog_sync_failed, result.error) to MaterialTheme.colorScheme.error
            }
            Text(
                text = text,
                style = MaterialTheme.typography.bodySmall,
                color = color
            )
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun CatalogSyncSectionPreviewIdle() {
    PezzottifyTheme {
        CatalogSyncSection(
            isResyncing = false,
            resyncResult = null,
            onForceResync = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun CatalogSyncSectionPreviewSyncing() {
    PezzottifyTheme {
        CatalogSyncSection(
            isResyncing = true,
            resyncResult = null,
            onForceResync = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun CatalogSyncSectionPreviewSuccess() {
    PezzottifyTheme {
        CatalogSyncSection(
            isResyncing = false,
            resyncResult = SkeletonResyncResult.Success,
            onForceResync = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun CatalogSyncSectionPreviewUpToDate() {
    PezzottifyTheme {
        CatalogSyncSection(
            isResyncing = false,
            resyncResult = SkeletonResyncResult.AlreadyUpToDate,
            onForceResync = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun CatalogSyncSectionPreviewFailed() {
    PezzottifyTheme {
        CatalogSyncSection(
            isResyncing = false,
            resyncResult = SkeletonResyncResult.Failed("Network error"),
            onForceResync = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}
