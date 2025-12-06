package com.lelloman.pezzottify.android.ui.screen.main.profile

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.model.StorageInfo
import com.lelloman.pezzottify.android.ui.model.StoragePressureLevel
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import kotlin.math.roundToInt

@Composable
fun StorageInfoSection(
    storageInfo: StorageInfo?,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier) {
        Text(
            text = "Storage",
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )

        Spacer(modifier = Modifier.height(16.dp))

        if (storageInfo != null) {
            val usedGB = storageInfo.usedBytes / (1024.0 * 1024.0 * 1024.0)
            val totalGB = storageInfo.totalBytes / (1024.0 * 1024.0 * 1024.0)
            val availableGB = storageInfo.availableBytes / (1024.0 * 1024.0 * 1024.0)
            val usedPercentage = (storageInfo.usedPercentage * 100).roundToInt()

            Column {
                Row(
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = "Used: ${String.format("%.2f", usedGB)} GB / ${String.format("%.2f", totalGB)} GB",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                }

                Spacer(modifier = Modifier.height(8.dp))

                LinearProgressIndicator(
                    progress = { storageInfo.usedPercentage.toFloat() },
                    modifier = Modifier.fillMaxWidth(),
                    color = when (storageInfo.pressureLevel) {
                        StoragePressureLevel.LOW -> MaterialTheme.colorScheme.primary
                        StoragePressureLevel.MEDIUM -> MaterialTheme.colorScheme.tertiary
                        StoragePressureLevel.HIGH -> MaterialTheme.colorScheme.error.copy(alpha = 0.7f)
                        StoragePressureLevel.CRITICAL -> MaterialTheme.colorScheme.error
                    }
                )

                Spacer(modifier = Modifier.height(8.dp))

                Row(
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = "Available: ${String.format("%.2f", availableGB)} GB",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(modifier = Modifier.weight(1f))
                    Text(
                        text = when (storageInfo.pressureLevel) {
                            StoragePressureLevel.LOW -> "Plenty of space"
                            StoragePressureLevel.MEDIUM -> "Moderate space"
                            StoragePressureLevel.HIGH -> "Low space"
                            StoragePressureLevel.CRITICAL -> "Very low space"
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = when (storageInfo.pressureLevel) {
                            StoragePressureLevel.LOW -> MaterialTheme.colorScheme.primary
                            StoragePressureLevel.MEDIUM -> MaterialTheme.colorScheme.tertiary
                            StoragePressureLevel.HIGH -> MaterialTheme.colorScheme.error.copy(alpha = 0.7f)
                            StoragePressureLevel.CRITICAL -> MaterialTheme.colorScheme.error
                        }
                    )
                }
            }
        } else {
            Text(
                text = "Storage information unavailable",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun StorageInfoSectionPreviewLow() {
    PezzottifyTheme {
        StorageInfoSection(
            storageInfo = StorageInfo(
                totalBytes = 128L * 1024L * 1024L * 1024L, // 128GB
                availableBytes = 100L * 1024L * 1024L * 1024L, // 100GB available
                usedBytes = 28L * 1024L * 1024L * 1024L, // 28GB used
                pressureLevel = StoragePressureLevel.LOW
            ),
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun StorageInfoSectionPreviewMedium() {
    PezzottifyTheme {
        StorageInfoSection(
            storageInfo = StorageInfo(
                totalBytes = 64L * 1024L * 1024L * 1024L, // 64GB
                availableBytes = 10L * 1024L * 1024L * 1024L, // 10GB available
                usedBytes = 54L * 1024L * 1024L * 1024L, // 54GB used
                pressureLevel = StoragePressureLevel.MEDIUM
            ),
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun StorageInfoSectionPreviewHigh() {
    PezzottifyTheme {
        StorageInfoSection(
            storageInfo = StorageInfo(
                totalBytes = 32L * 1024L * 1024L * 1024L, // 32GB
                availableBytes = 2L * 1024L * 1024L * 1024L, // 2GB available
                usedBytes = 30L * 1024L * 1024L * 1024L, // 30GB used
                pressureLevel = StoragePressureLevel.HIGH
            ),
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun StorageInfoSectionPreviewCritical() {
    PezzottifyTheme {
        StorageInfoSection(
            storageInfo = StorageInfo(
                totalBytes = 16L * 1024L * 1024L * 1024L, // 16GB
                availableBytes = 512L * 1024L * 1024L, // 512MB available
                usedBytes = 15L * 1024L * 1024L * 1024L + 512L * 1024L * 1024L, // ~15.5GB used
                pressureLevel = StoragePressureLevel.CRITICAL
            ),
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun StorageInfoSectionPreviewNull() {
    PezzottifyTheme {
        StorageInfoSection(
            storageInfo = null,
            modifier = Modifier.padding(16.dp)
        )
    }
}
