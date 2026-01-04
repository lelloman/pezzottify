package com.lelloman.simpleaiassistant.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.lelloman.simpleaiassistant.model.Language

@Composable
fun LanguagePickerDialog(
    selectedLanguage: Language?,
    onLanguageSelected: (Language?) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Response Language") },
        text = {
            LazyColumn(
                modifier = Modifier.height(400.dp)
            ) {
                // Auto-detect option
                item {
                    LanguageOption(
                        flag = "?",
                        displayText = "Auto-detect",
                        subText = "Let the assistant detect your language",
                        isSelected = selectedLanguage == null,
                        onClick = { onLanguageSelected(null) }
                    )
                }

                items(Language.entries) { language ->
                    LanguageOption(
                        flag = language.flag,
                        displayText = language.nativeName,
                        subText = language.displayName,
                        isSelected = selectedLanguage == language,
                        onClick = { onLanguageSelected(language) }
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("Done")
            }
        },
        modifier = modifier
    )
}

@Composable
private fun LanguageOption(
    flag: String,
    displayText: String,
    subText: String,
    isSelected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        RadioButton(
            selected = isSelected,
            onClick = onClick
        )
        Spacer(modifier = Modifier.width(4.dp))
        Text(
            text = flag,
            style = MaterialTheme.typography.titleLarge
        )
        Spacer(modifier = Modifier.width(12.dp))
        Column {
            Text(
                text = displayText,
                style = MaterialTheme.typography.bodyLarge
            )
            Text(
                text = subText,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
