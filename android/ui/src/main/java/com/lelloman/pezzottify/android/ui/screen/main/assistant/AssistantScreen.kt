package com.lelloman.pezzottify.android.ui.screen.main.assistant

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.hilt.navigation.compose.hiltViewModel
import com.lelloman.simpleaiassistant.ui.ChatScreen

@Composable
fun AssistantScreen(
    viewModel: AssistantViewModel = hiltViewModel()
) {
    val state by viewModel.uiState.collectAsState()

    ChatScreen(
        state = state,
        onSendMessage = viewModel::sendMessage,
        onClearHistory = viewModel::clearHistory,
        onOpenSettings = { /* TODO: Open settings dialog */ },
        onRestartFromMessage = viewModel::restartFromMessage,
        onLanguageSelected = viewModel::setLanguage
    )
}
