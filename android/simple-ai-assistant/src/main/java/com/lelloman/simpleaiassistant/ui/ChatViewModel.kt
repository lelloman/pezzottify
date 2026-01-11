package com.lelloman.simpleaiassistant.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.simpleaiassistant.data.ChatRepository
import com.lelloman.simpleaiassistant.mode.AssistantMode
import com.lelloman.simpleaiassistant.mode.ModeManager
import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.Language
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

class ChatViewModel(
    private val chatRepository: ChatRepository,
    private val modeManager: ModeManager? = null
) : ViewModel() {

    private val _debugMode = MutableStateFlow(false)
    private val _error = MutableStateFlow<String?>(null)

    private val _uiState = MutableStateFlow(ChatUiState())
    val uiState: StateFlow<ChatUiState> = _uiState.asStateFlow()

    init {
        viewModelScope.launch {
            // First combine: messages, streaming, and language (5 flows max)
            val baseRepoFlow = combine(
                chatRepository.messages,
                chatRepository.streamingText,
                chatRepository.isStreaming,
                chatRepository.language,
                chatRepository.isDetectingLanguage
            ) { messages: List<ChatMessage>, streamingText: String, isStreaming: Boolean, language: Language?, isDetectingLanguage: Boolean ->
                BaseRepoState(messages, streamingText, isStreaming, language, isDetectingLanguage)
            }

            // Second combine: add mode to base state
            val repoFlow = combine(
                baseRepoFlow,
                chatRepository.currentMode
            ) { baseState, currentMode ->
                RepoState(
                    baseState.messages,
                    baseState.streamingText,
                    baseState.isStreaming,
                    baseState.language,
                    baseState.isDetectingLanguage,
                    currentMode
                )
            }

            // Third combine: add local state
            combine(
                repoFlow,
                _debugMode,
                _error
            ) { repoState, debugMode, error ->
                // Get mode path from manager if available
                val modePath = if (repoState.currentMode != null && modeManager != null) {
                    modeManager.getCurrentPath()
                } else {
                    emptyList()
                }

                ChatUiState(
                    messages = repoState.messages,
                    streamingText = repoState.streamingText,
                    isStreaming = repoState.isStreaming,
                    language = repoState.language,
                    isDetectingLanguage = repoState.isDetectingLanguage,
                    debugMode = debugMode,
                    error = error,
                    currentMode = repoState.currentMode,
                    allModes = modeManager?.getAllModes() ?: emptyList(),
                    modePath = modePath
                )
            }.collect { state ->
                _uiState.value = state
            }
        }
    }

    private data class BaseRepoState(
        val messages: List<ChatMessage>,
        val streamingText: String,
        val isStreaming: Boolean,
        val language: Language?,
        val isDetectingLanguage: Boolean
    )

    private data class RepoState(
        val messages: List<ChatMessage>,
        val streamingText: String,
        val isStreaming: Boolean,
        val language: Language?,
        val isDetectingLanguage: Boolean,
        val currentMode: AssistantMode?
    )

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
        _debugMode.update { !it }
    }

    fun clearError() {
        _error.value = null
    }

    fun restartFromMessage(messageId: String) {
        viewModelScope.launch {
            _error.value = null
            try {
                chatRepository.restartFromMessage(messageId)
            } catch (e: Exception) {
                _error.value = e.message ?: "Unknown error"
            }
        }
    }

    fun switchMode(modeId: String) {
        viewModelScope.launch {
            _error.value = null
            try {
                val success = chatRepository.switchMode(modeId)
                if (!success) {
                    _error.value = "Failed to switch to mode: $modeId"
                }
            } catch (e: Exception) {
                _error.value = e.message ?: "Unknown error"
            }
        }
    }
}
