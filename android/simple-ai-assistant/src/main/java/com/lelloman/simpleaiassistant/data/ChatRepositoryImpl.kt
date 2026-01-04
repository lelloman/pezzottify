package com.lelloman.simpleaiassistant.data

import com.lelloman.simpleaiassistant.data.local.ChatMessageDao
import com.lelloman.simpleaiassistant.data.local.ChatMessageEntity
import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.Language
import com.lelloman.simpleaiassistant.model.MessageRole
import com.lelloman.simpleaiassistant.model.StreamEvent
import com.lelloman.simpleaiassistant.tool.ToolRegistry
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import java.util.UUID

class ChatRepositoryImpl(
    private val chatMessageDao: ChatMessageDao,
    private val llmProvider: LlmProvider,
    private val toolRegistry: ToolRegistry,
    private val systemPromptBuilder: SystemPromptBuilder
) : ChatRepository {

    private val _streamingText = MutableStateFlow("")
    override val streamingText: StateFlow<String> = _streamingText.asStateFlow()

    private val _isStreaming = MutableStateFlow(false)
    override val isStreaming: StateFlow<Boolean> = _isStreaming.asStateFlow()

    private val _language = MutableStateFlow<Language?>(null)
    override val language: StateFlow<Language?> = _language.asStateFlow()

    override val messages: Flow<List<ChatMessage>> = chatMessageDao.observeAll()
        .map { entities -> entities.map { it.toDomain() } }

    override suspend fun sendMessage(text: String) {
        // 1. Save user message
        val userMessage = ChatMessage(
            id = generateId(),
            role = MessageRole.USER,
            content = text
        )
        saveMessage(userMessage)

        // 2. Detect language if not set
        if (_language.value == null) {
            detectAndSetLanguage(text)
        }

        // 3. Get response from LLM
        _isStreaming.value = true
        _streamingText.value = ""

        try {
            processLlmResponse()
        } finally {
            _isStreaming.value = false
            _streamingText.value = ""
        }
    }

    private suspend fun processLlmResponse() {
        val currentMessages = chatMessageDao.getAll().map { it.toDomain() }
        val systemPrompt = systemPromptBuilder.build(_language.value, toolRegistry)

        var assistantContent = StringBuilder()
        val toolCalls = mutableListOf<com.lelloman.simpleaiassistant.model.ToolCall>()

        llmProvider.streamChat(
            messages = currentMessages,
            tools = toolRegistry.getRootSpecs(),
            systemPrompt = systemPrompt
        ).collect { event ->
            when (event) {
                is StreamEvent.Text -> {
                    assistantContent.append(event.content)
                    _streamingText.value = assistantContent.toString()
                }
                is StreamEvent.ToolUse -> {
                    toolCalls.add(
                        com.lelloman.simpleaiassistant.model.ToolCall(
                            id = event.id,
                            name = event.name,
                            input = event.input
                        )
                    )
                }
                is StreamEvent.Error -> {
                    // Save error as assistant message
                    val errorMessage = ChatMessage(
                        id = generateId(),
                        role = MessageRole.ASSISTANT,
                        content = "Error: ${event.message}"
                    )
                    saveMessage(errorMessage)
                    return@collect
                }
                is StreamEvent.Done -> { /* handled below */ }
            }
        }

        // 4. Save assistant message
        val assistantMessage = ChatMessage(
            id = generateId(),
            role = MessageRole.ASSISTANT,
            content = assistantContent.toString(),
            toolCalls = toolCalls.takeIf { it.isNotEmpty() }
        )
        saveMessage(assistantMessage)

        // 5. Execute tool calls if any
        if (toolCalls.isNotEmpty()) {
            for (toolCall in toolCalls) {
                val tool = toolRegistry.findById(toolCall.name)
                val result = tool?.execute(toolCall.input)
                    ?: com.lelloman.simpleaiassistant.tool.ToolResult(
                        success = false,
                        error = "Tool not found: ${toolCall.name}"
                    )

                // Save tool result message
                val toolResultMessage = ChatMessage(
                    id = generateId(),
                    role = MessageRole.TOOL,
                    content = resultToString(result),
                    toolCallId = toolCall.id,
                    toolName = toolCall.name
                )
                saveMessage(toolResultMessage)
            }

            // 6. Get follow-up response after tool execution
            processLlmResponse()
        }
    }

    private fun resultToString(result: com.lelloman.simpleaiassistant.tool.ToolResult): String {
        return if (result.success) {
            result.data?.toString() ?: "Success"
        } else {
            "Error: ${result.error ?: "Unknown error"}"
        }
    }

    private suspend fun detectAndSetLanguage(text: String) {
        val detectedCode = llmProvider.detectLanguage(text)
        if (detectedCode != null) {
            _language.value = Language.fromCode(detectedCode)
        }
    }

    override suspend fun setLanguage(language: Language?) {
        _language.value = language
    }

    override suspend fun clearHistory() {
        chatMessageDao.deleteAll()
    }

    private suspend fun saveMessage(message: ChatMessage) {
        chatMessageDao.insert(ChatMessageEntity.fromDomain(message))
    }

    private fun generateId(): String = UUID.randomUUID().toString()
}

/**
 * Builds the system prompt for the LLM.
 */
fun interface SystemPromptBuilder {
    fun build(language: Language?, toolRegistry: ToolRegistry): String
}
