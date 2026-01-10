package com.lelloman.pezzottify.android.ui.screen.main.assistant

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.simpleaiassistant.ui.ChatScreen
import com.lelloman.simpleaiassistant.ui.ProviderSettingsDialog

@Composable
fun AssistantScreen(
    viewModel: AssistantViewModel = hiltViewModel()
) {
    val state by viewModel.uiState.collectAsState()
    val currentProviderId by viewModel.providerConfigStore.selectedProviderId.collectAsState()
    val currentConfig by viewModel.providerConfigStore.config.collectAsState()
    var showSettings by remember { mutableStateOf(false) }

    ChatScreen(
        state = state,
        onSendMessage = viewModel::sendMessage,
        onClearHistory = viewModel::clearHistory,
        onOpenSettings = { showSettings = true },
        onRestartFromMessage = viewModel::restartFromMessage,
        onLanguageSelected = viewModel::setLanguage
    )

    if (showSettings) {
        ProviderSettingsDialog(
            registry = viewModel.providerRegistry,
            currentProviderId = currentProviderId,
            currentConfig = currentConfig,
            onSave = { providerId, config ->
                viewModel.saveProviderSettings(providerId, config)
                showSettings = false
            },
            onDismiss = { showSettings = false }
        )
    }
}
