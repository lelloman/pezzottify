package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun ServerSection(
    baseUrl: String,
    baseUrlInput: String,
    baseUrlError: String?,
    isSaving: Boolean,
    onBaseUrlInputChanged: (String) -> Unit,
    onSaveBaseUrl: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val hasChanges = baseUrlInput.trim() != baseUrl

    Column(modifier = modifier) {
        Text(
            text = "Server",
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )

        Spacer(modifier = Modifier.height(16.dp))

        OutlinedTextField(
            value = baseUrlInput,
            onValueChange = onBaseUrlInputChanged,
            label = { Text("Server URL") },
            placeholder = { Text("http://example.com:3001") },
            isError = baseUrlError != null,
            supportingText = {
                when {
                    baseUrlError != null -> Text(
                        text = baseUrlError,
                        color = MaterialTheme.colorScheme.error
                    )
                    hasChanges -> Text(
                        text = "Press Save to apply changes",
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    else -> Text(
                        text = "Current server address",
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            },
            singleLine = true,
            enabled = !isSaving,
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Uri,
                imeAction = ImeAction.Done
            ),
            keyboardActions = KeyboardActions(
                onDone = { if (hasChanges && !isSaving) onSaveBaseUrl() }
            ),
            modifier = Modifier.fillMaxWidth()
        )

        Spacer(modifier = Modifier.height(8.dp))

        Button(
            onClick = onSaveBaseUrl,
            enabled = hasChanges && !isSaving && baseUrlError == null,
            modifier = Modifier.fillMaxWidth()
        ) {
            if (isSaving) {
                CircularProgressIndicator(
                    modifier = Modifier.height(16.dp),
                    strokeWidth = 2.dp,
                    color = MaterialTheme.colorScheme.onPrimary
                )
            } else {
                Text("Save")
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = "Changing the server URL will affect where the app connects. Make sure the URL is correct before saving.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ServerSectionPreviewUnchanged() {
    PezzottifyTheme {
        ServerSection(
            baseUrl = "http://10.0.2.2:3001",
            baseUrlInput = "http://10.0.2.2:3001",
            baseUrlError = null,
            isSaving = false,
            onBaseUrlInputChanged = {},
            onSaveBaseUrl = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ServerSectionPreviewWithChanges() {
    PezzottifyTheme {
        ServerSection(
            baseUrl = "http://10.0.2.2:3001",
            baseUrlInput = "http://192.168.1.100:3001",
            baseUrlError = null,
            isSaving = false,
            onBaseUrlInputChanged = {},
            onSaveBaseUrl = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ServerSectionPreviewWithError() {
    PezzottifyTheme {
        ServerSection(
            baseUrl = "http://10.0.2.2:3001",
            baseUrlInput = "invalid-url",
            baseUrlError = "Invalid URL",
            isSaving = false,
            onBaseUrlInputChanged = {},
            onSaveBaseUrl = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ServerSectionPreviewSaving() {
    PezzottifyTheme {
        ServerSection(
            baseUrl = "http://10.0.2.2:3001",
            baseUrlInput = "http://192.168.1.100:3001",
            baseUrlError = null,
            isSaving = true,
            onBaseUrlInputChanged = {},
            onSaveBaseUrl = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ServerSectionPreviewDark() {
    PezzottifyTheme(darkTheme = true) {
        ServerSection(
            baseUrl = "http://10.0.2.2:3001",
            baseUrlInput = "http://192.168.1.100:3001",
            baseUrlError = null,
            isSaving = false,
            onBaseUrlInputChanged = {},
            onSaveBaseUrl = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}
