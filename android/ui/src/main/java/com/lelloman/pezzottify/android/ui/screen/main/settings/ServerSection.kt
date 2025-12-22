package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun ServerSection(
    baseUrl: String,
    baseUrlInput: String,
    @StringRes baseUrlErrorRes: Int?,
    isSaving: Boolean,
    onBaseUrlInputChanged: (String) -> Unit,
    onSaveBaseUrl: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val hasChanges = baseUrlInput.trim() != baseUrl

    Column(modifier = modifier) {
        Text(
            text = stringResource(R.string.server),
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )

        Spacer(modifier = Modifier.height(16.dp))

        OutlinedTextField(
            value = baseUrlInput,
            onValueChange = onBaseUrlInputChanged,
            label = { Text(stringResource(R.string.server_url)) },
            placeholder = { Text(stringResource(R.string.server_url_placeholder)) },
            isError = baseUrlErrorRes != null,
            supportingText = {
                when {
                    baseUrlErrorRes != null -> Text(
                        text = stringResource(baseUrlErrorRes),
                        color = MaterialTheme.colorScheme.error
                    )
                    hasChanges -> Text(
                        text = stringResource(R.string.press_save_to_apply),
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    else -> Text(
                        text = stringResource(R.string.current_server_address),
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
            enabled = hasChanges && !isSaving && baseUrlErrorRes == null,
            modifier = Modifier.fillMaxWidth()
        ) {
            if (isSaving) {
                PezzottifyLoader(
                    size = LoaderSize.Button,
                    color = MaterialTheme.colorScheme.onPrimary
                )
            } else {
                Text(stringResource(R.string.save))
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = stringResource(R.string.server_url_change_warning),
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
            baseUrlErrorRes = null,
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
            baseUrlErrorRes = null,
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
            baseUrlErrorRes = R.string.invalid_url,
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
            baseUrlErrorRes = null,
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
            baseUrlErrorRes = null,
            isSaving = false,
            onBaseUrlInputChanged = {},
            onSaveBaseUrl = {},
            modifier = Modifier.padding(16.dp)
        )
    }
}
