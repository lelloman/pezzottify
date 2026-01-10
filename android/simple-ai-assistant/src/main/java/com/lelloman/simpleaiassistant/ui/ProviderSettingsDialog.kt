package com.lelloman.simpleaiassistant.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.lelloman.simpleaiassistant.llm.ConfigField
import com.lelloman.simpleaiassistant.llm.LlmProviderFactory
import com.lelloman.simpleaiassistant.llm.ProviderRegistry
import kotlinx.coroutines.launch

/**
 * Settings dialog for configuring LLM providers.
 *
 * If registry has single provider, shows that provider's config directly.
 * If registry has multiple providers, shows provider picker first.
 */
@Composable
fun ProviderSettingsDialog(
    registry: ProviderRegistry,
    currentProviderId: String?,
    currentConfig: Map<String, Any?>,
    onSave: (providerId: String, config: Map<String, Any?>) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier
) {
    val singleFactory = registry.getSingleFactory()

    if (singleFactory != null) {
        // Single provider - show config directly
        SingleProviderSettingsDialog(
            factory = singleFactory,
            currentConfig = currentConfig,
            onSave = { config -> onSave(singleFactory.providerId, config) },
            onDismiss = onDismiss,
            modifier = modifier
        )
    } else {
        // Multiple providers - show picker
        MultiProviderSettingsDialog(
            registry = registry,
            currentProviderId = currentProviderId,
            currentConfig = currentConfig,
            onSave = onSave,
            onDismiss = onDismiss,
            modifier = modifier
        )
    }
}

@Composable
private fun SingleProviderSettingsDialog(
    factory: LlmProviderFactory,
    currentConfig: Map<String, Any?>,
    onSave: (Map<String, Any?>) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier
) {
    val config = remember { mutableStateMapOf<String, Any?>() }
    var testResult by remember { mutableStateOf<TestResult?>(null) }
    var isTesting by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    // Initialize config with current or default values
    LaunchedEffect(Unit) {
        val defaults = factory.getDefaultConfig()
        defaults.forEach { (key, value) ->
            config[key] = currentConfig[key] ?: value
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("${factory.displayName} Settings") },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
            ) {
                factory.configSchema.fields.forEach { field ->
                    ConfigFieldInput(
                        field = field,
                        value = config[field.key],
                        onValueChange = { config[field.key] = it },
                        onFetchOptions = { fieldKey ->
                            factory.fetchDynamicOptions(fieldKey, config.toMap())
                        }
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                }

                // Test connection button and result
                Spacer(modifier = Modifier.height(8.dp))
                TestConnectionRow(
                    isTesting = isTesting,
                    testResult = testResult,
                    onTest = {
                        scope.launch {
                            isTesting = true
                            testResult = null
                            val result = factory.testConnection(config.toMap())
                            testResult = if (result.isSuccess) {
                                TestResult.Success
                            } else {
                                TestResult.Error(result.exceptionOrNull()?.message ?: "Unknown error")
                            }
                            isTesting = false
                        }
                    }
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    val error = factory.validateConfig(config.toMap())
                    if (error == null) {
                        onSave(config.toMap())
                    }
                }
            ) {
                Text("Save")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        },
        modifier = modifier
    )
}

@Composable
private fun MultiProviderSettingsDialog(
    registry: ProviderRegistry,
    currentProviderId: String?,
    currentConfig: Map<String, Any?>,
    onSave: (providerId: String, config: Map<String, Any?>) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier
) {
    var selectedProviderId by remember { mutableStateOf(currentProviderId ?: registry.getProviderIds().firstOrNull()) }
    val selectedFactory = selectedProviderId?.let { registry.getFactory(it) }

    val config = remember { mutableStateMapOf<String, Any?>() }
    var testResult by remember { mutableStateOf<TestResult?>(null) }
    var isTesting by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    // Reset config when provider changes
    LaunchedEffect(selectedProviderId) {
        config.clear()
        testResult = null
        selectedFactory?.let { factory ->
            val defaults = factory.getDefaultConfig()
            val configToUse = if (selectedProviderId == currentProviderId) currentConfig else emptyMap()
            defaults.forEach { (key, value) ->
                config[key] = configToUse[key] ?: value
            }
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("LLM Provider Settings") },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
            ) {
                // Provider picker
                Text(
                    text = "Provider",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(4.dp))
                ProviderPicker(
                    providers = registry.getFactories(),
                    selectedId = selectedProviderId,
                    onSelect = { selectedProviderId = it }
                )

                Spacer(modifier = Modifier.height(16.dp))

                // Provider-specific config fields
                selectedFactory?.let { factory ->
                    factory.configSchema.fields.forEach { field ->
                        ConfigFieldInput(
                            field = field,
                            value = config[field.key],
                            onValueChange = { config[field.key] = it },
                            onFetchOptions = { fieldKey ->
                                factory.fetchDynamicOptions(fieldKey, config.toMap())
                            }
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                    }

                    // Test connection
                    Spacer(modifier = Modifier.height(8.dp))
                    TestConnectionRow(
                        isTesting = isTesting,
                        testResult = testResult,
                        onTest = {
                            scope.launch {
                                isTesting = true
                                testResult = null
                                val result = factory.testConnection(config.toMap())
                                testResult = if (result.isSuccess) {
                                    TestResult.Success
                                } else {
                                    TestResult.Error(result.exceptionOrNull()?.message ?: "Unknown error")
                                }
                                isTesting = false
                            }
                        }
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    if (selectedProviderId != null && selectedFactory != null) {
                        val error = selectedFactory.validateConfig(config.toMap())
                        if (error == null) {
                            onSave(selectedProviderId!!, config.toMap())
                        }
                    }
                }
            ) {
                Text("Save")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        },
        modifier = modifier
    )
}

@Composable
private fun ProviderPicker(
    providers: List<LlmProviderFactory>,
    selectedId: String?,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    var expanded by remember { mutableStateOf(false) }
    val selectedProvider = providers.find { it.providerId == selectedId }

    Box(modifier = modifier) {
        OutlinedTextField(
            value = selectedProvider?.displayName ?: "",
            onValueChange = {},
            readOnly = true,
            trailingIcon = {
                Icon(
                    Icons.Default.KeyboardArrowDown,
                    contentDescription = "Select provider"
                )
            },
            modifier = Modifier
                .fillMaxWidth()
                .clickable { expanded = true }
        )
        DropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false }
        ) {
            providers.forEach { provider ->
                DropdownMenuItem(
                    text = { Text(provider.displayName) },
                    onClick = {
                        onSelect(provider.providerId)
                        expanded = false
                    }
                )
            }
        }
    }
}

@Composable
private fun ConfigFieldInput(
    field: ConfigField,
    value: Any?,
    onValueChange: (Any?) -> Unit,
    onFetchOptions: suspend (String) -> List<String>,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier.fillMaxWidth()) {
        when (field) {
            is ConfigField.Text -> TextFieldInput(field, value, onValueChange)
            is ConfigField.Number -> NumberFieldInput(field, value, onValueChange)
            is ConfigField.Select -> SelectFieldInput(field, value, onValueChange, onFetchOptions)
            is ConfigField.Toggle -> ToggleFieldInput(field, value, onValueChange)
        }
    }
}

@Composable
private fun TextFieldInput(
    field: ConfigField.Text,
    value: Any?,
    onValueChange: (Any?) -> Unit
) {
    OutlinedTextField(
        value = (value as? String) ?: "",
        onValueChange = { onValueChange(it) },
        label = { Text(field.label) },
        placeholder = { Text(field.placeholder) },
        supportingText = field.description?.let { { Text(it) } },
        visualTransformation = if (field.isSecret) PasswordVisualTransformation() else VisualTransformation.None,
        singleLine = true,
        modifier = Modifier.fillMaxWidth()
    )
}

@Composable
private fun NumberFieldInput(
    field: ConfigField.Number,
    value: Any?,
    onValueChange: (Any?) -> Unit
) {
    val textValue = (value as? Number)?.toString() ?: ""

    OutlinedTextField(
        value = textValue,
        onValueChange = { newValue ->
            val number = newValue.toLongOrNull()
            if (number != null || newValue.isEmpty()) {
                onValueChange(number)
            }
        },
        label = { Text(field.label) },
        supportingText = field.description?.let { { Text(it) } },
        suffix = field.suffix?.let { { Text(it) } },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        singleLine = true,
        modifier = Modifier.fillMaxWidth()
    )
}

@Composable
private fun SelectFieldInput(
    field: ConfigField.Select,
    value: Any?,
    onValueChange: (Any?) -> Unit,
    onFetchOptions: suspend (String) -> List<String>
) {
    var expanded by remember { mutableStateOf(false) }
    var options by remember { mutableStateOf(field.options) }
    var isLoading by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val currentValue = (value as? String) ?: ""

    Column {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(modifier = Modifier.weight(1f)) {
                OutlinedTextField(
                    value = currentValue,
                    onValueChange = { if (field.allowCustom) onValueChange(it) },
                    label = { Text(field.label) },
                    supportingText = field.description?.let { { Text(it) } },
                    readOnly = !field.allowCustom,
                    trailingIcon = {
                        Row {
                            if (field.dynamicOptions) {
                                IconButton(
                                    onClick = {
                                        scope.launch {
                                            isLoading = true
                                            options = onFetchOptions(field.key)
                                            isLoading = false
                                            if (options.isNotEmpty()) {
                                                expanded = true
                                            }
                                        }
                                    }
                                ) {
                                    if (isLoading) {
                                        CircularProgressIndicator(
                                            modifier = Modifier.size(20.dp),
                                            strokeWidth = 2.dp
                                        )
                                    } else {
                                        Icon(Icons.Default.Refresh, "Fetch options")
                                    }
                                }
                            }
                            IconButton(onClick = { expanded = true }) {
                                Icon(Icons.Default.KeyboardArrowDown, "Select")
                            }
                        }
                    },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                DropdownMenu(
                    expanded = expanded && options.isNotEmpty(),
                    onDismissRequest = { expanded = false }
                ) {
                    options.forEach { option ->
                        DropdownMenuItem(
                            text = { Text(option) },
                            onClick = {
                                onValueChange(option)
                                expanded = false
                            }
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ToggleFieldInput(
    field: ConfigField.Toggle,
    value: Any?,
    onValueChange: (Any?) -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = field.label,
                style = MaterialTheme.typography.bodyLarge
            )
            field.description?.let {
                Text(
                    text = it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
        Switch(
            checked = (value as? Boolean) ?: field.default,
            onCheckedChange = { onValueChange(it) }
        )
    }
}

@Composable
private fun TestConnectionRow(
    isTesting: Boolean,
    testResult: TestResult?,
    onTest: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        TextButton(
            onClick = onTest,
            enabled = !isTesting
        ) {
            if (isTesting) {
                CircularProgressIndicator(
                    modifier = Modifier.size(16.dp),
                    strokeWidth = 2.dp
                )
                Spacer(modifier = Modifier.width(8.dp))
            }
            Text("Test Connection")
        }

        testResult?.let { result ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                when (result) {
                    is TestResult.Success -> {
                        Icon(
                            Icons.Default.Check,
                            contentDescription = "Success",
                            tint = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.size(20.dp)
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(
                            "Connected",
                            color = MaterialTheme.colorScheme.primary,
                            style = MaterialTheme.typography.bodySmall
                        )
                    }
                    is TestResult.Error -> {
                        Icon(
                            Icons.Default.Close,
                            contentDescription = "Error",
                            tint = MaterialTheme.colorScheme.error,
                            modifier = Modifier.size(20.dp)
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(
                            result.message,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                            maxLines = 1
                        )
                    }
                }
            }
        }
    }
}

private sealed class TestResult {
    data object Success : TestResult()
    data class Error(val message: String) : TestResult()
}
