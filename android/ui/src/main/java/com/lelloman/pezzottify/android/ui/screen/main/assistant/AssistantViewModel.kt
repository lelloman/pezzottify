package com.lelloman.pezzottify.android.ui.screen.main.assistant

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.simpleaiassistant.data.ChatRepository
import com.lelloman.simpleaiassistant.model.Language
import com.lelloman.simpleaiassistant.ui.ChatUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class AssistantViewModel @Inject constructor(
    private val chatRepository: ChatRepository
) : ViewModel() {

    private val _debugMode = MutableStateFlow(false)
    private val _error = MutableStateFlow<String?>(null)

    private val _uiState = MutableStateFlow(ChatUiState())
    val uiState: StateFlow<ChatUiState> = _uiState.asStateFlow()

    init {
        viewModelScope.launch {
            // Combine repository flows into a data class first (max 5 flows at once)
            data class RepoState(
                val messages: List<com.lelloman.simpleaiassistant.model.ChatMessage>,
                val streamingText: String,
                val isStreaming: Boolean,
                val language: Language?
            )

            val repoFlow = combine(
                chatRepository.messages,
                chatRepository.streamingText,
                chatRepository.isStreaming,
                chatRepository.language
            ) { messages, streamingText, isStreaming, language ->
                RepoState(messages, streamingText, isStreaming, language)
            }

            // Combine with local state
            combine(
                repoFlow,
                _debugMode,
                _error
            ) { repoState, debugMode, error ->
                ChatUiState(
                    messages = repoState.messages,
                    streamingText = repoState.streamingText,
                    isStreaming = repoState.isStreaming,
                    language = repoState.language,
                    debugMode = debugMode,
                    error = error
                )
            }.collect { state ->
                _uiState.value = state
            }
        }
    }

    fun sendMessage(text: String) {
        if (text.isBlank()) return

        viewModelScope.launch {
            _error.value = null
            try {
                chatRepository.sendMessage(text)
            } catch (e: Exception) {
                _error.value = e.message ?: "Unknown error"
            }
        }
    }

    fun setLanguage(language: Language?) {
        viewModelScope.launch {
            chatRepository.setLanguage(language)
        }
    }

    fun clearHistory() {
        viewModelScope.launch {
            chatRepository.clearHistory()
        }
    }

    fun toggleDebugMode() {
        _debugMode.value = !_debugMode.value
    }

    fun clearError() {
        _error.value = null
    }
}
