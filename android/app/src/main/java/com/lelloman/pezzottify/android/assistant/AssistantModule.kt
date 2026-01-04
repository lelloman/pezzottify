package com.lelloman.pezzottify.android.assistant

import android.content.Context
import androidx.room.Room
import com.lelloman.simpleaiassistant.data.ChatRepository
import com.lelloman.simpleaiassistant.data.ChatRepositoryImpl
import com.lelloman.simpleaiassistant.data.DefaultSystemPromptBuilder
import com.lelloman.simpleaiassistant.data.SystemPromptBuilder
import com.lelloman.simpleaiassistant.data.local.ChatDatabase
import com.lelloman.simpleaiassistant.data.local.ChatMessageDao
import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.provider.ollama.OllamaConfig
import com.lelloman.simpleaiassistant.provider.ollama.OllamaProvider
import com.lelloman.simpleaiassistant.tool.ToolRegistry
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object AssistantModule {

    @Provides
    @Singleton
    fun provideChatDatabase(@ApplicationContext context: Context): ChatDatabase {
        return Room.databaseBuilder(
            context,
            ChatDatabase::class.java,
            "chat_database"
        ).build()
    }

    @Provides
    @Singleton
    fun provideChatMessageDao(database: ChatDatabase): ChatMessageDao {
        return database.chatMessageDao()
    }

    @Provides
    @Singleton
    fun provideOllamaConfig(): OllamaConfig {
        // TODO: Make this configurable via settings
        return OllamaConfig(
            baseUrl = "http://192.168.1.92:11434",
            model = "gpt-oss:20b",
            timeoutMs = 120_000L
        )
    }

    @Provides
    @Singleton
    fun provideLlmProvider(config: OllamaConfig): LlmProvider {
        return OllamaProvider(config)
    }

    @Provides
    @Singleton
    fun provideToolRegistry(): ToolRegistry {
        // Start with an empty tool registry
        // TODO: Add Pezzottify-specific tools (playback, search, etc.)
        return ToolRegistry(
            tools = emptyMap(),
            topography = emptyList()
        )
    }

    @Provides
    @Singleton
    fun provideSystemPromptBuilder(): SystemPromptBuilder {
        return DefaultSystemPromptBuilder(
            assistantName = "Pezzottify Assistant",
            additionalInstructions = """
                You are an AI assistant for a music streaming app called Pezzottify.
                You can help users discover music, manage playlists, and answer questions about artists and albums.
            """.trimIndent()
        )
    }

    @Provides
    @Singleton
    fun provideChatRepository(
        chatMessageDao: ChatMessageDao,
        llmProvider: LlmProvider,
        toolRegistry: ToolRegistry,
        systemPromptBuilder: SystemPromptBuilder
    ): ChatRepository {
        return ChatRepositoryImpl(
            chatMessageDao = chatMessageDao,
            llmProvider = llmProvider,
            toolRegistry = toolRegistry,
            systemPromptBuilder = systemPromptBuilder
        )
    }
}
