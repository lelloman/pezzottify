package com.lelloman.pezzottify.android.ui.component.dialog

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R

const val DEFAULT_DESTINATION_STEPS = 10
private const val MAX_DESTINATION_STEPS = 500

/** Asks over how many suggested tracks the queue should drift toward [label]. */
@Composable
fun SetDestinationDialog(
    label: String,
    initialSteps: Int = DEFAULT_DESTINATION_STEPS,
    onDismiss: () -> Unit,
    onConfirm: (steps: Int) -> Unit,
) {
    var text by remember { mutableStateOf(initialSteps.toString()) }
    val steps = text.toIntOrNull()?.takeIf { it in 1..MAX_DESTINATION_STEPS }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = stringResource(R.string.steering_steps_title),
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
            )
        },
        text = {
            Column {
                Text(
                    text = stringResource(R.string.steering_steps_prompt, label),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(modifier = Modifier.height(16.dp))
                OutlinedTextField(
                    value = text,
                    onValueChange = { value -> text = value.filter { it.isDigit() }.take(3) },
                    label = { Text(stringResource(R.string.steering_steps_label)) },
                    singleLine = true,
                    isError = steps == null,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    textStyle = MaterialTheme.typography.headlineMedium.copy(
                        fontWeight = FontWeight.Bold,
                        textAlign = TextAlign.Center,
                    ),
                    shape = RoundedCornerShape(16.dp),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            Button(
                onClick = {
                    steps?.let {
                        onConfirm(it)
                        onDismiss()
                    }
                },
                enabled = steps != null,
                shape = CircleShape,
            ) {
                Text(stringResource(R.string.steering_set))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, shape = CircleShape) {
                Text(stringResource(R.string.cancel))
            }
        },
    )
}
