package com.lelloman.simpleaiassistant.data

import com.lelloman.simpleaiassistant.data.local.ChatMessageDao
import com.lelloman.simpleaiassistant.data.local.ChatMessageEntity
import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.mode.AssistantMode
import com.lelloman.simpleaiassistant.mode.DefaultHistoryCompactor
import com.lelloman.simpleaiassistant.mode.HistoryCompactor
import com.lelloman.simpleaiassistant.mode.ModeManager
import com.lelloman.simpleaiassistant.mode.SwitchModeTool
import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.Language
import com.lelloman.simpleaiassistant.model.MessageRole
import com.lelloman.simpleaiassistant.model.StreamEvent
import com.lelloman.simpleaiassistant.tool.Tool
import com.lelloman.simpleaiassistant.tool.ToolRegistry
import com.lelloman.simpleaiassistant.R
import com.lelloman.simpleaiassistant.util.AssistantLogger
import com.lelloman.simpleaiassistant.util.AuthErrorHandler
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
    private val logger: AssistantLogger = NoOpAssistantLogger,
    private val authErrorHandler: AuthErrorHandler = AuthErrorHandler.NoOp,
    private val modeManager: ModeManager? = null,
    private val historyCompactor: HistoryCompactor = DefaultHistoryCompactor()
) : ChatRepository {

    companion object {
        private const val TAG = "ChatRepository"
        private const val MAX_TOOL_ITERATIONS = 10
        private const val AUTH_ERROR_PREFIX = "Authentication failed"
    }

    private val _streamingText = MutableStateFlow("")
    override val streamingText: StateFlow<String> = _streamingText.asStateFlow()

    private val _isStreaming = MutableStateFlow(false)
    override val isStreaming: StateFlow<Boolean> = _isStreaming.asStateFlow()

    private val _language = MutableStateFlow(languagePreferences.getLanguage())
    override val language: StateFlow<Language?> = _language.asStateFlow()

    private val _isDetectingLanguage = MutableStateFlow(false)
    override val isDetectingLanguage: StateFlow<Boolean> = _isDetectingLanguage.asStateFlow()

    // Mode support - null if modes not configured
    private val _currentMode = MutableStateFlow(modeManager?.currentMode?.value)
    override val currentMode: StateFlow<AssistantMode?> = _currentMode.asStateFlow()

    // Switch mode tool - created lazily if mode manager is available
    private val switchModeTool: SwitchModeTool? = modeManager?.let {
        SwitchModeTool(it) { fromMode, toMode ->
            handleModeSwitch(fromMode, toMode)
        }
    }

    // Context summary from history compaction (prepended to system prompt)
    private var contextSummary: String? = null

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

    private suspend fun processLlmResponse(iteration: Int, isAuthRetry: Boolean = false) {
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
        val systemPrompt = buildModeAwareSystemPrompt()
        val toolSpecs = getCurrentToolSpecs()

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
        var authErrorMessage: String? = null

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
                    // Check if this is an auth error
                    if (event.message.contains(AUTH_ERROR_PREFIX)) {
                        authErrorMessage = event.message
                    } else {
                        // Non-auth error, save as message
                        val errorMessage = ChatMessage(
                            id = generateId(),
                            role = MessageRole.ASSISTANT,
                            content = "Error: ${event.message}"
                        )
                        saveMessage(errorMessage)
                    }
                    return@collect
                }
                is StreamEvent.Done -> {
                    logger.debug(TAG, "<<< LLM STREAM DONE")
                }
            }
        }

        // Handle auth error with retry
        if (!isAuthRetry) {
            authErrorMessage?.let { message ->
                logger.info(TAG, "Auth error detected, attempting token refresh and retry")
                val shouldRetry = authErrorHandler.onAuthError(message)
                if (shouldRetry) {
                    logger.info(TAG, "Token refreshed, retrying LLM request")
                    processLlmResponse(iteration, isAuthRetry = true)
                    return
                } else {
                    logger.warn(TAG, "Auth error handler did not refresh tokens, showing error to user")
                    val errorMessage = ChatMessage(
                        id = generateId(),
                        role = MessageRole.ASSISTANT,
                        content = "Error: $message"
                    )
                    saveMessage(errorMessage)
                    return
                }
            }
        } else if (authErrorMessage != null) {
            // Already retried once, show error
            logger.warn(TAG, "Auth error persists after retry, showing error to user")
            val errorMessage = ChatMessage(
                id = generateId(),
                role = MessageRole.ASSISTANT,
                content = "Error: $authErrorMessage"
            )
            saveMessage(errorMessage)
            return
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

                val tool = findTool(toolCall.name)
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
        logger.debug(TAG, "detectAndSetLanguage: starting detection for text: ${text.take(50)}")
        _isDetectingLanguage.value = true
        try {
            val detectedCode = llmProvider.detectLanguage(text)
            logger.debug(TAG, "detectAndSetLanguage: LLM returned code: $detectedCode")
            if (detectedCode != null) {
                val language = Language.fromCode(detectedCode)
                logger.debug(TAG, "detectAndSetLanguage: mapped to language: $language")
                _language.value = language
                languagePreferences.setLanguage(language)
                logger.info(TAG, "Language detected and set: ${language?.displayName ?: "null"}")
            } else {
                logger.warn(TAG, "detectAndSetLanguage: LLM returned null, language not set")
            }
        } catch (e: Exception) {
            logger.error(TAG, "detectAndSetLanguage: exception during detection", e)
        } finally {
            _isDetectingLanguage.value = false
        }
    }

    override suspend fun setLanguage(language: Language?) {
        logger.info(TAG, "setLanguage: ${language?.displayName ?: "null (auto-detect)"}")
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

    // ==================== Mode Support ====================

    override suspend fun switchMode(modeId: String): Boolean {
        val manager = modeManager ?: return false
        val fromMode = manager.currentMode.value
        val targetMode = manager.getModeTree().findMode(modeId) ?: return false

        if (fromMode.id == modeId) {
            return true // Already in this mode
        }

        // Compact history before switching
        handleModeSwitch(fromMode, targetMode)

        // Actually switch the mode
        val success = manager.switchToMode(modeId)
        if (success) {
            _currentMode.value = manager.currentMode.value
        }
        return success
    }

    private suspend fun handleModeSwitch(fromMode: AssistantMode, toMode: AssistantMode) {
        logger.info(TAG, "Mode switch: ${fromMode.id} -> ${toMode.id}")

        val currentMessages = chatMessageDao.getAll().map { it.toDomain() }
        if (currentMessages.isEmpty()) {
            contextSummary = null
            return
        }

        val compacted = historyCompactor.compact(currentMessages, fromMode, toMode)

        // Clear all messages and re-add only the kept ones
        chatMessageDao.deleteAll()
        compacted.keptMessages.forEach { msg ->
            chatMessageDao.insert(ChatMessageEntity.fromDomain(msg))
        }

        // Store context summary for system prompt
        contextSummary = compacted.contextSummary

        // Update current mode state
        _currentMode.value = toMode

        logger.info(TAG, "History compacted: ${currentMessages.size} -> ${compacted.keptMessages.size} messages")
        if (compacted.contextSummary != null) {
            logger.debug(TAG, "Context summary: ${compacted.contextSummary}")
        }
    }

    /**
     * Builds the system prompt, optionally including mode-specific instructions.
     */
    private fun buildModeAwareSystemPrompt(): String {
        val basePrompt = systemPromptBuilder.build(_language.value, toolRegistry)

        val mode = _currentMode.value
        if (mode == null) {
            // No mode configured, use base prompt with optional context summary
            return if (contextSummary != null) {
                "$basePrompt\n\n## Previous Context\n$contextSummary"
            } else {
                basePrompt
            }
        }

        // Build mode-aware prompt
        return buildString {
            append(basePrompt)

            appendLine()
            appendLine()
            appendLine("## Current Mode: ${mode.name}")
            appendLine(mode.description)

            if (mode.promptInstructions.isNotBlank()) {
                appendLine()
                appendLine(mode.promptInstructions)
            }

            // Show all available modes
            modeManager?.let { manager ->
                val allModes = manager.getAllModes()
                if (allModes.size > 1) {
                    appendLine()
                    appendLine("## Available Modes")
                    appendLine("To switch modes, call the switch_mode tool with action='switch' and mode_id='<id>'.")
                    appendLine("Do NOT just mention modes in text - actually call the tool to switch.")
                    allModes.forEach { m ->
                        val current = if (m.id == mode.id) " (current)" else ""
                        appendLine("- ${m.id}: ${m.name}$current")
                    }
                }
            }

            // Add context summary if present
            if (contextSummary != null) {
                appendLine()
                appendLine("## Previous Context")
                appendLine(contextSummary)
            }
        }
    }

    /**
     * Gets the tool specs available in the current mode.
     * If modes are not configured, returns all root tools.
     */
    private fun getCurrentToolSpecs(): List<com.lelloman.simpleaiassistant.tool.ToolSpec> {
        val mode = _currentMode.value
        if (mode == null) {
            // No mode configured, return all root tools
            return toolRegistry.getRootSpecs()
        }

        // Get tools for current mode
        val modeToolSpecs = mode.toolIds.mapNotNull { toolId ->
            toolRegistry.findById(toolId)?.spec
        }

        // Always include switch_mode tool if available
        val switchModeSpec = switchModeTool?.spec

        return if (switchModeSpec != null) {
            modeToolSpecs + switchModeSpec
        } else {
            modeToolSpecs
        }
    }

    /**
     * Finds a tool by name, checking both the registry and built-in tools.
     */
    private fun findTool(toolName: String): Tool? {
        // Check built-in switch_mode tool first
        if (toolName == SwitchModeTool.TOOL_NAME && switchModeTool != null) {
            return switchModeTool
        }
        // Check registry
        return toolRegistry.findById(toolName)
    }
}

/**
 * Builds the system prompt for the LLM.
 */
fun interface SystemPromptBuilder {
    fun build(language: Language?, toolRegistry: ToolRegistry): String
}
