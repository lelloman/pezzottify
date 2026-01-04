package com.lelloman.simpleaiassistant.data

import com.lelloman.simpleaiassistant.data.local.ChatMessageDao
import com.lelloman.simpleaiassistant.data.local.ChatMessageEntity
import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.Language
import com.lelloman.simpleaiassistant.model.MessageRole
import com.lelloman.simpleaiassistant.model.StreamEvent
import com.lelloman.simpleaiassistant.tool.ToolRegistry
import com.lelloman.simpleaiassistant.R
import com.lelloman.simpleaiassistant.util.AssistantLogger
import com.lelloman.simpleaiassistant.util.LanguagePreferences
import com.lelloman.simpleaiassistant.util.NoOpAssistantLogger
import com.lelloman.simpleaiassistant.util.StringProvider
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
    private val systemPromptBuilder: SystemPromptBuilder,
    private val stringProvider: StringProvider,
    private val languagePreferences: LanguagePreferences,
    private val logger: AssistantLogger = NoOpAssistantLogger
) : ChatRepository {

    companion object {
        private const val TAG = "ChatRepository"
        private const val MAX_TOOL_ITERATIONS = 10
    }

    private val _streamingText = MutableStateFlow("")
    override val streamingText: StateFlow<String> = _streamingText.asStateFlow()

    private val _isStreaming = MutableStateFlow(false)
    override val isStreaming: StateFlow<Boolean> = _isStreaming.asStateFlow()

    private val _language = MutableStateFlow(languagePreferences.getLanguage())
    override val language: StateFlow<Language?> = _language.asStateFlow()

    private val _isDetectingLanguage = MutableStateFlow(false)
    override val isDetectingLanguage: StateFlow<Boolean> = _isDetectingLanguage.asStateFlow()

    override val messages: Flow<List<ChatMessage>> = chatMessageDao.observeAll()
        .map { entities -> entities.map { it.toDomain() } }

    override suspend fun sendMessage(text: String) {
        logger.info(TAG, "sendMessage: \"${text.take(100)}${if (text.length > 100) "..." else ""}\"")

        // 1. Save user message
        val userMessage = ChatMessage(
            id = generateId(),
            role = MessageRole.USER,
            content = text
        )
        saveMessage(userMessage)

        // 2. Detect language if not set
        if (_language.value == null) {
            logger.debug(TAG, "Language not set, detecting...")
            detectAndSetLanguage(text)
            logger.debug(TAG, "Detected language: ${_language.value}")
        }

        // 3. Get response from LLM
        _isStreaming.value = true
        _streamingText.value = ""

        try {
            processLlmResponse(iteration = 0)
        } catch (e: Exception) {
            logger.error(TAG, "Error processing LLM response", e)
            throw e
        } finally {
            _isStreaming.value = false
            _streamingText.value = ""
        }
    }

    private suspend fun processLlmResponse(iteration: Int) {
        if (iteration >= MAX_TOOL_ITERATIONS) {
            logger.warn(TAG, "Max tool iterations ($MAX_TOOL_ITERATIONS) reached, stopping")
            val errorMessage = ChatMessage(
                id = generateId(),
                role = MessageRole.ASSISTANT,
                content = stringProvider.getString(R.string.error_max_tool_iterations)
            )
            saveMessage(errorMessage)
            return
        }
        val currentMessages = chatMessageDao.getAll().map { it.toDomain() }
        val systemPrompt = systemPromptBuilder.build(_language.value, toolRegistry)
        val toolSpecs = toolRegistry.getRootSpecs()

        // Log full conversation being sent to LLM
        logger.info(TAG, "=== SENDING TO LLM ===")
        logger.info(TAG, "System prompt (${systemPrompt.length} chars):\n$systemPrompt")
        logger.info(TAG, "Tools available: ${toolSpecs.map { it.name }}")
        logger.info(TAG, "Messages (${currentMessages.size}):")
        currentMessages.forEach { msg ->
            val content = msg.content.take(500) + if (msg.content.length > 500) "..." else ""
            val toolInfo = when {
                msg.toolCalls != null -> " [tool_calls: ${msg.toolCalls.map { "${it.name}(${it.input})" }}]"
                msg.toolName != null -> " [tool_response for: ${msg.toolName}]"
                else -> ""
            }
            logger.info(TAG, "  [${msg.role}]$toolInfo: $content")
        }
        logger.info(TAG, "=== END SENDING ===")

        var assistantContent = StringBuilder()
        val toolCalls = mutableListOf<com.lelloman.simpleaiassistant.model.ToolCall>()

        llmProvider.streamChat(
            messages = currentMessages,
            tools = toolSpecs,
            systemPrompt = systemPrompt
        ).collect { event ->
            when (event) {
                is StreamEvent.Text -> {
                    assistantContent.append(event.content)
                    _streamingText.value = assistantContent.toString()
                }
                is StreamEvent.ToolUse -> {
                    logger.info(TAG, "<<< LLM TOOL CALL: ${event.name}")
                    logger.info(TAG, "    Input: ${event.input}")
                    toolCalls.add(
                        com.lelloman.simpleaiassistant.model.ToolCall(
                            id = event.id,
                            name = event.name,
                            input = event.input
                        )
                    )
                }
                is StreamEvent.Error -> {
                    logger.error(TAG, "<<< LLM ERROR: ${event.message}")
                    // Save error as assistant message
                    val errorMessage = ChatMessage(
                        id = generateId(),
                        role = MessageRole.ASSISTANT,
                        content = "Error: ${event.message}"
                    )
                    saveMessage(errorMessage)
                    return@collect
                }
                is StreamEvent.Done -> {
                    logger.debug(TAG, "<<< LLM STREAM DONE")
                }
            }
        }

        // Log full assistant response
        logger.info(TAG, "=== LLM RESPONSE ===")
        logger.info(TAG, "Text (${assistantContent.length} chars): $assistantContent")
        if (toolCalls.isNotEmpty()) {
            logger.info(TAG, "Tool calls (${toolCalls.size}):")
            toolCalls.forEach { tc ->
                logger.info(TAG, "  - ${tc.name}: ${tc.input}")
            }
        }
        logger.info(TAG, "=== END RESPONSE ===")

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
                logger.info(TAG, ">>> EXECUTING TOOL: ${toolCall.name}")
                logger.info(TAG, "    Input: ${toolCall.input}")

                val tool = toolRegistry.findById(toolCall.name)
                if (tool == null) {
                    logger.error(TAG, "    ERROR: Tool not found!")
                }
                val result = tool?.execute(toolCall.input)
                    ?: com.lelloman.simpleaiassistant.tool.ToolResult(
                        success = false,
                        error = "Tool not found: ${toolCall.name}"
                    )

                logger.info(TAG, "<<< TOOL RESULT: ${toolCall.name}")
                logger.info(TAG, "    Success: ${result.success}")
                logger.info(TAG, "    Data: ${result.data}")
                if (result.error != null) {
                    logger.info(TAG, "    Error: ${result.error}")
                }

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
            logger.info(TAG, "--- Continuing conversation after tool execution (iteration ${iteration + 1}) ---")
            processLlmResponse(iteration + 1)
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
        _isDetectingLanguage.value = true
        try {
            val detectedCode = llmProvider.detectLanguage(text)
            if (detectedCode != null) {
                val language = Language.fromCode(detectedCode)
                _language.value = language
                languagePreferences.setLanguage(language)
            }
        } finally {
            _isDetectingLanguage.value = false
        }
    }

    override suspend fun setLanguage(language: Language?) {
        _language.value = language
        languagePreferences.setLanguage(language)
    }

    override suspend fun clearHistory() {
        logger.info(TAG, "Clearing chat history")
        chatMessageDao.deleteAll()
    }

    override suspend fun restartFromMessage(messageId: String) {
        logger.info(TAG, "Restarting from message: $messageId")

        val message = chatMessageDao.getById(messageId)
        if (message == null) {
            logger.error(TAG, "Message not found: $messageId")
            return
        }

        // Delete all messages after this one (including this one, we'll re-add it)
        chatMessageDao.deleteAfterTimestamp(message.timestamp - 1)

        // Re-send the message to get a new response
        sendMessage(message.content)
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
