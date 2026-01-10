package com.lelloman.pezzottify.android.assistant

import android.content.Context
import androidx.room.Room
import com.lelloman.pezzottify.android.assistant.tools.GetArtistDiscographyTool
import com.lelloman.pezzottify.android.assistant.tools.NowPlayingTool
import com.lelloman.pezzottify.android.assistant.tools.PlayAlbumTool
import com.lelloman.pezzottify.android.assistant.tools.PlaybackControlTool
import com.lelloman.pezzottify.android.assistant.tools.PlaybackModeTool
import com.lelloman.pezzottify.android.assistant.tools.QueueTool
import com.lelloman.pezzottify.android.assistant.tools.SearchCatalogTool
import com.lelloman.pezzottify.android.assistant.tools.SkipTrackTool
import com.lelloman.pezzottify.android.assistant.tools.WhatsNewTool
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.statics.DiscographyProvider
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWhatsNew
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.simpleaiassistant.util.AssistantLogger
import com.lelloman.simpleaiassistant.model.Language
import com.lelloman.simpleaiassistant.util.LanguagePreferences
import com.lelloman.simpleaiassistant.util.StringProvider
import com.lelloman.simpleaiassistant.data.ChatRepository
import com.lelloman.simpleaiassistant.data.ChatRepositoryImpl
import com.lelloman.simpleaiassistant.data.DefaultSystemPromptBuilder
import com.lelloman.simpleaiassistant.data.SystemPromptBuilder
import com.lelloman.simpleaiassistant.data.local.ChatDatabase
import com.lelloman.simpleaiassistant.data.local.ChatMessageDao
import com.lelloman.simpleaiassistant.llm.DynamicLlmProvider
import com.lelloman.simpleaiassistant.llm.LlmProvider
import com.lelloman.simpleaiassistant.llm.ProviderConfigStore
import com.lelloman.simpleaiassistant.llm.ProviderRegistry
import com.lelloman.simpleaiassistant.provider.ollama.OllamaProviderFactory
import com.lelloman.simpleaiassistant.tool.ToolNode
import com.lelloman.simpleaiassistant.tool.ToolRegistry
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.CoroutineScope
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
    fun provideProviderRegistry(): ProviderRegistry {
        return ProviderRegistry(
            OllamaProviderFactory()
            // Add more providers here when available:
            // AnthropicProviderFactory(),
            // OpenAiProviderFactory(),
        )
    }

    @Provides
    @Singleton
    fun provideProviderConfigStore(@ApplicationContext context: Context): ProviderConfigStore {
        return SharedPrefsProviderConfigStore(context)
    }

    @Provides
    @Singleton
    fun provideLlmProvider(
        registry: ProviderRegistry,
        configStore: ProviderConfigStore,
        scope: CoroutineScope
    ): LlmProvider {
        return DynamicLlmProvider(registry, configStore, scope)
    }

    @Provides
    @Singleton
    fun provideToolRegistry(
        player: PezzottifyPlayer,
        metadataProvider: PlaybackMetadataProvider,
        performSearch: PerformSearch,
        staticsStore: StaticsStore,
        discographyProvider: DiscographyProvider,
        getWhatsNew: GetWhatsNew
    ): ToolRegistry {
        // Create all the tools
        val playbackControlTool = PlaybackControlTool(player)
        val skipTrackTool = SkipTrackTool(player)
        val playbackModeTool = PlaybackModeTool(player)
        val nowPlayingTool = NowPlayingTool(player, metadataProvider)
        val queueTool = QueueTool(player, metadataProvider)
        val playAlbumTool = PlayAlbumTool(player)
        val searchCatalogTool = SearchCatalogTool(performSearch, staticsStore)
        val getArtistDiscographyTool = GetArtistDiscographyTool(discographyProvider, staticsStore)
        val whatsNewTool = WhatsNewTool(getWhatsNew)

        val allTools = listOf(
            playbackControlTool,
            skipTrackTool,
            playbackModeTool,
            nowPlayingTool,
            queueTool,
            playAlbumTool,
            searchCatalogTool,
            getArtistDiscographyTool,
            whatsNewTool
        )

        return ToolRegistry(
            tools = allTools.associateBy { it.spec.name },
            topography = listOf(
                // Most common tools at root level - always available
                ToolNode.ToolRef(playbackControlTool.spec.name),
                ToolNode.ToolRef(skipTrackTool.spec.name),
                ToolNode.ToolRef(nowPlayingTool.spec.name),
                ToolNode.ToolRef(searchCatalogTool.spec.name),
                ToolNode.ToolRef(getArtistDiscographyTool.spec.name),
                ToolNode.ToolRef(whatsNewTool.spec.name),
                ToolNode.ToolRef(playAlbumTool.spec.name),
                // Less common tools in groups - can be expanded when needed
                ToolNode.Group(
                    id = "playback_modes",
                    name = "Playback Modes",
                    description = "Shuffle and repeat settings",
                    children = listOf(
                        ToolNode.ToolRef(playbackModeTool.spec.name)
                    )
                ),
                ToolNode.Group(
                    id = "queue_management",
                    name = "Queue Management",
                    description = "View and manage the playback queue",
                    children = listOf(
                        ToolNode.ToolRef(queueTool.spec.name)
                    )
                )
            )
        )
    }

    @Provides
    @Singleton
    fun provideSystemPromptBuilder(): SystemPromptBuilder {
        return DefaultSystemPromptBuilder(
            assistantName = "Pezzottify Assistant",
            additionalInstructions = """
                You are an AI assistant for a music streaming app called Pezzottify.
                You can help users discover music, control playback, manage the queue, and answer questions about artists and albums.

                You have access to the following tools:
                - playback_control: Play, pause, or stop music
                - skip_track: Skip to next or previous track
                - playback_mode: Toggle shuffle and repeat modes
                - now_playing: Get information about the currently playing track
                - queue: View or manage the playback queue
                - search_catalog: Search for tracks, albums, and artists
                - get_artist_discography: Get an artist's albums by artist ID
                - whats_new: Get latest releases and recent catalog additions
                - play_album: Play an album by its ID

                When the user asks to play music by an artist:
                1. Use search_catalog to find the artist
                2. Use get_artist_discography with the artist ID to get their albums
                3. Use play_album with one of the album IDs to play it

                When the user asks about new music, latest releases, or what's new, use the whats_new tool.
                Always check now_playing first if the user asks about the current song.
                Be helpful and concise in your responses.
            """.trimIndent()
        )
    }

    @Provides
    @Singleton
    fun provideAssistantLogger(loggerFactory: LoggerFactory): AssistantLogger {
        return PezzottifyAssistantLogger(loggerFactory)
    }

    @Provides
    @Singleton
    fun provideStringProvider(@ApplicationContext context: Context): StringProvider {
        return object : StringProvider {
            override fun getString(resId: Int): String = context.getString(resId)
        }
    }

    @Provides
    @Singleton
    fun provideLanguagePreferences(@ApplicationContext context: Context): LanguagePreferences {
        val prefs = context.getSharedPreferences("assistant_prefs", Context.MODE_PRIVATE)
        return object : LanguagePreferences {
            override fun getLanguage(): Language? {
                val code = prefs.getString("language_code", null)
                return code?.let { Language.fromCode(it) }
            }

            override fun setLanguage(language: Language?) {
                prefs.edit().apply {
                    if (language != null) {
                        putString("language_code", language.code)
                    } else {
                        remove("language_code")
                    }
                    apply()
                }
            }
        }
    }

    @Provides
    @Singleton
    fun provideChatRepository(
        chatMessageDao: ChatMessageDao,
        llmProvider: LlmProvider,
        toolRegistry: ToolRegistry,
        systemPromptBuilder: SystemPromptBuilder,
        stringProvider: StringProvider,
        languagePreferences: LanguagePreferences,
        logger: AssistantLogger
    ): ChatRepository {
        return ChatRepositoryImpl(
            chatMessageDao = chatMessageDao,
            llmProvider = llmProvider,
            toolRegistry = toolRegistry,
            systemPromptBuilder = systemPromptBuilder,
            stringProvider = stringProvider,
            languagePreferences = languagePreferences,
            logger = logger
        )
    }
}
