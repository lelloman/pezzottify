package com.lelloman.pezzottify.android.ui.screen.main.assistant

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.lelloman.pezzottify.android.ui.R
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.simpleaiassistant.ui.ChatScreen
import com.lelloman.simpleaiassistant.ui.SettingsBottomSheet

@Composable
fun AssistantScreen(
    viewModel: AssistantViewModel = hiltViewModel(),
    onReportMessage: (String)->Unit = {},
) {
    val state by viewModel.uiState.collectAsState()
    val confirmation by viewModel.confirmation.request.collectAsState()
    confirmation?.let { request ->
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { viewModel.confirmation.respond(request.id, false) },
            title = { androidx.compose.material3.Text(stringResource(R.string.ai_confirm_action)) },
            text = { androidx.compose.material3.Text("${request.action}\n\n${request.details}") },
            confirmButton = {
                androidx.compose.material3.TextButton(onClick = { viewModel.confirmation.respond(request.id, true) }) {
                    androidx.compose.material3.Text(stringResource(R.string.ai_allow_action))
                }
            },
            dismissButton = {
                androidx.compose.material3.TextButton(onClick = { viewModel.confirmation.respond(request.id, false) }) {
                    androidx.compose.material3.Text(stringResource(R.string.cancel))
                }
            },
        )
    }
    val currentProviderId by viewModel.providerConfigStore.selectedProviderId.collectAsState()
    val currentConfig by viewModel.providerConfigStore.config.collectAsState()
    var showSettings by remember { mutableStateOf(false) }

    ChatScreen(
        state = state,
        onSendMessage = viewModel::sendMessage,
        onClearHistory = viewModel::clearHistory,
        onOpenSettings = { showSettings = true },
        onRestartFromMessage = viewModel::restartFromMessage,
        onLanguageSelected = viewModel::setLanguage,
        onModeSelected = viewModel::switchMode,
        onReportMessage = onReportMessage,
        reportLabel = stringResource(R.string.feedback_report_response),
        onCancel = viewModel::cancel,
        onConfirmRestart = viewModel::confirmRestart,
        onDismissRestart = viewModel::dismissRestart
    )

    if (showSettings) {
        SettingsBottomSheet(
            registry = viewModel.providerRegistry,
            currentProviderId = currentProviderId,
            currentConfig = currentConfig,
            debugMode = state.debugMode,
            showProviderSettings = viewModel.isProviderConfigurationVisible,
            onDebugModeChange = { viewModel.setDebugMode(it) },
            onSave = { providerId, config ->
                viewModel.saveProviderSettings(providerId, config)
            },
            onDismiss = { showSettings = false }
        )
    }
}
