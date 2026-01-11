package com.lelloman.pezzottify.android.ui.screen.main.assistant

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.simpleaiassistant.data.ChatRepository
import com.lelloman.simpleaiassistant.llm.ProviderConfigStore
import com.lelloman.simpleaiassistant.llm.ProviderRegistry
import com.lelloman.simpleaiassistant.mode.AssistantMode
import com.lelloman.simpleaiassistant.mode.ModeManager
import com.lelloman.simpleaiassistant.model.Language
import com.lelloman.simpleaiassistant.ui.ChatUiState
import com.lelloman.simpleaiassistant.util.DebugModePreferences
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class AssistantViewModel @Inject constructor(
    private val chatRepository: ChatRepository,
    val providerRegistry: ProviderRegistry,
    val providerConfigStore: ProviderConfigStore,
    private val modeManager: ModeManager,
    private val debugModePreferences: DebugModePreferences
) : ViewModel() {

    private val _debugMode = MutableStateFlow(debugModePreferences.isDebugMode())
    private val _error = MutableStateFlow<String?>(null)

    private val _uiState = MutableStateFlow(ChatUiState())
    val uiState: StateFlow<ChatUiState> = _uiState.asStateFlow()

    init {
        viewModelScope.launch {
            // Combine repository flows in two steps (max 5 flows at once)
            data class BaseRepoState(
                val messages: List<com.lelloman.simpleaiassistant.model.ChatMessage>,
                val streamingText: String,
                val isStreaming: Boolean,
                val language: Language?,
                val isDetectingLanguage: Boolean
            )

            data class RepoState(
                val messages: List<com.lelloman.simpleaiassistant.model.ChatMessage>,
                val streamingText: String,
                val isStreaming: Boolean,
                val language: Language?,
                val isDetectingLanguage: Boolean,
                val currentMode: AssistantMode?
            )

            // First combine: base repository state (5 flows)
            val baseRepoFlow = combine(
                chatRepository.messages,
                chatRepository.streamingText,
                chatRepository.isStreaming,
                chatRepository.language,
                chatRepository.isDetectingLanguage
            ) { messages, streamingText, isStreaming, language, isDetectingLanguage ->
                BaseRepoState(messages, streamingText, isStreaming, language, isDetectingLanguage)
            }

            // Second combine: add mode
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
                // Get mode path from manager
                val modePath = if (repoState.currentMode != null) {
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
                    allModes = modeManager.getAllModes(),
                    modePath = modePath
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
        val newValue = !_debugMode.value
        _debugMode.value = newValue
        debugModePreferences.setDebugMode(newValue)
    }

    fun setDebugMode(enabled: Boolean) {
        _debugMode.value = enabled
        debugModePreferences.setDebugMode(enabled)
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

    fun saveProviderSettings(providerId: String, config: Map<String, Any?>) {
        viewModelScope.launch {
            providerConfigStore.save(providerId, config)
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
