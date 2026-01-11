package com.lelloman.pezzottify.android.ui.screen.main.assistant

import androidx.compose.runtime.Composable
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
        onLanguageSelected = viewModel::setLanguage,
        onModeSelected = viewModel::switchMode
    )

    if (showSettings) {
        SettingsBottomSheet(
            registry = viewModel.providerRegistry,
            currentProviderId = currentProviderId,
            currentConfig = currentConfig,
            debugMode = state.debugMode,
            onDebugModeChange = { viewModel.setDebugMode(it) },
            onSave = { providerId, config ->
                viewModel.saveProviderSettings(providerId, config)
            },
            onDismiss = { showSettings = false }
        )
    }
}
