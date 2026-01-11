package com.lelloman.pezzottify.android.assistant

import android.content.Context
import androidx.room.Room
import com.lelloman.pezzottify.android.assistant.tools.AddPlaylistToQueueTool
import com.lelloman.pezzottify.android.assistant.tools.AddTracksToPlaylistTool
import com.lelloman.pezzottify.android.assistant.tools.CreatePlaylistTool
import com.lelloman.pezzottify.android.assistant.tools.DeletePlaylistTool
import com.lelloman.pezzottify.android.assistant.tools.GetArtistDiscographyTool
import com.lelloman.pezzottify.android.assistant.tools.ListPlaylistsTool
import com.lelloman.pezzottify.android.assistant.tools.NowPlayingTool
import com.lelloman.pezzottify.android.assistant.tools.PlayAlbumTool
import com.lelloman.pezzottify.android.assistant.tools.PlayPlaylistTool
import com.lelloman.pezzottify.android.assistant.tools.PlaybackControlTool
import com.lelloman.pezzottify.android.assistant.tools.PlaybackModeTool
import com.lelloman.pezzottify.android.assistant.tools.QueueTool
import com.lelloman.pezzottify.android.assistant.tools.RemoveTracksFromPlaylistTool
import com.lelloman.pezzottify.android.assistant.tools.RenamePlaylistTool
import com.lelloman.pezzottify.android.assistant.tools.SearchCatalogTool
import com.lelloman.pezzottify.android.assistant.tools.SkipTrackTool
import com.lelloman.pezzottify.android.assistant.tools.ViewPlaylistTool
import com.lelloman.pezzottify.android.assistant.tools.WhatsNewTool
import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.statics.DiscographyProvider
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWhatsNew
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.simpleaiassistant.util.AssistantLogger
import com.lelloman.simpleaiassistant.util.AuthErrorHandler
import com.lelloman.simpleaiassistant.util.DebugModePreferences
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
import com.lelloman.simpleaiassistant.mode.ModeManager
import com.lelloman.simpleaiassistant.mode.ModePreferences
import com.lelloman.simpleaiassistant.mode.ModeTree
import com.lelloman.simpleaiassistant.provider.ollama.OllamaProviderFactory
import com.lelloman.simpleaiassistant.provider.simpleai.SimpleAiProviderFactory
import com.lelloman.simpleaiassistant.tool.ToolNode
import com.lelloman.simpleaiassistant.tool.ToolRegistry
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
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
    fun provideProviderRegistry(
        @ApplicationContext context: Context,
        authStore: AuthStore
    ): ProviderRegistry {
        return ProviderRegistry(
            OllamaProviderFactory(),
            SimpleAiProviderFactory(
                context = context,
                authTokenProvider = {
                    when (val state = authStore.getAuthState().value) {
                        is AuthState.LoggedIn -> state.authToken
                        else -> null
                    }
                }
            ),
            // Add more providers here when available:
            // AnthropicProviderFactory(),
            // OpenAiProviderFactory(),
            defaultProviderId = "simpleai"
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
        getWhatsNew: GetWhatsNew,
        userPlaylistStore: UserPlaylistStore
    ): ToolRegistry {
        // Catalog/Search tools
        val searchCatalogTool = SearchCatalogTool(performSearch, staticsStore)
        val getArtistDiscographyTool = GetArtistDiscographyTool(discographyProvider, staticsStore)
        val whatsNewTool = WhatsNewTool(getWhatsNew)

        // Playback control tools
        val playbackControlTool = PlaybackControlTool(player)
        val skipTrackTool = SkipTrackTool(player)
        val playbackModeTool = PlaybackModeTool(player)
        val nowPlayingTool = NowPlayingTool(player, metadataProvider)
        val queueTool = QueueTool(player, metadataProvider)
        val playAlbumTool = PlayAlbumTool(player)

        // Playlist tools
        val listPlaylistsTool = ListPlaylistsTool(userPlaylistStore)
        val viewPlaylistTool = ViewPlaylistTool(userPlaylistStore, staticsStore)
        val createPlaylistTool = CreatePlaylistTool(userPlaylistStore)
        val renamePlaylistTool = RenamePlaylistTool(userPlaylistStore)
        val deletePlaylistTool = DeletePlaylistTool(userPlaylistStore)
        val addTracksToPlaylistTool = AddTracksToPlaylistTool(userPlaylistStore, staticsStore)
        val removeTracksFromPlaylistTool = RemoveTracksFromPlaylistTool(userPlaylistStore, staticsStore)
        val playPlaylistTool = PlayPlaylistTool(player, userPlaylistStore)
        val addPlaylistToQueueTool = AddPlaylistToQueueTool(player, userPlaylistStore)

        val allTools = listOf(
            // Catalog tools
            searchCatalogTool,
            getArtistDiscographyTool,
            whatsNewTool,
            // Playback tools
            playbackControlTool,
            skipTrackTool,
            playbackModeTool,
            nowPlayingTool,
            queueTool,
            playAlbumTool,
            // Playlist tools
            listPlaylistsTool,
            viewPlaylistTool,
            createPlaylistTool,
            renamePlaylistTool,
            deletePlaylistTool,
            addTracksToPlaylistTool,
            removeTracksFromPlaylistTool,
            playPlaylistTool,
            addPlaylistToQueueTool
        )

        return ToolRegistry(
            tools = allTools.associateBy { it.spec.name },
            topography = allTools.map { ToolNode.ToolRef(it.spec.name) }
        )
    }

    @Provides
    @Singleton
    fun provideSystemPromptBuilder(): SystemPromptBuilder {
        return DefaultSystemPromptBuilder(
            assistantName = "Pezzottify Assistant",
            additionalInstructions = """
                You are an AI assistant for Pezzottify, a music streaming app.
                Help users discover music, control playback, manage queues, and explore the catalog.
                Be helpful and concise.

                CRITICAL - ID Rules:
                - Always use IDs exactly as returned by tools. Never guess or make up IDs.
                - Track IDs are for queue operations and individual track playback.
                - Album IDs are for play_album and get_artist_discography results.
                - Artist IDs are for get_artist_discography input.

                Common Workflows:

                Play music by artist:
                1. search_catalog to find the artist → get artist ID
                2. get_artist_discography with artist ID → get album IDs
                3. play_album with album ID

                Play a specific track:
                1. search_catalog with filter="tracks" → get track ID
                2. queue action="add_track" with track ID

                Current song questions:
                - Always use now_playing first to get current playback state.

                New music / latest releases:
                - Use whats_new to show recent catalog additions.

                Content Availability:
                - By default, search only returns playable content.
                - Use include_unavailable=true for discovery/exploration.
                - Unavailable content cannot be played but may interest the user.
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
    fun provideDebugModePreferences(@ApplicationContext context: Context): DebugModePreferences {
        val prefs = context.getSharedPreferences("assistant_prefs", Context.MODE_PRIVATE)
        return object : DebugModePreferences {
            override fun isDebugMode(): Boolean {
                return prefs.getBoolean("debug_mode", false)
            }

            override fun setDebugMode(enabled: Boolean) {
                prefs.edit().putBoolean("debug_mode", enabled).apply()
            }
        }
    }

    @Provides
    @Singleton
    fun provideAuthErrorHandler(
        tokenRefresher: TokenRefresher,
        loggerFactory: LoggerFactory
    ): AuthErrorHandler {
        val logger = loggerFactory.getLogger("AuthErrorHandler")
        return AuthErrorHandler { errorMessage ->
            logger.info("Auth error received, attempting token refresh: $errorMessage")
            when (val result = tokenRefresher.refreshTokens()) {
                is TokenRefresher.RefreshResult.Success -> {
                    logger.info("Token refresh successful")
                    true
                }
                is TokenRefresher.RefreshResult.Failed -> {
                    logger.warn("Token refresh failed: ${result.reason}")
                    false
                }
                is TokenRefresher.RefreshResult.NotAvailable -> {
                    logger.warn("Token refresh not available (no refresh token)")
                    false
                }
                is TokenRefresher.RefreshResult.RateLimited -> {
                    logger.warn("Token refresh rate limited, retry after ${result.retryAfterMs}ms")
                    false
                }
            }
        }
    }

    @Provides
    @Singleton
    fun provideModeTree(): ModeTree {
        return PezzottifyModeTree.create()
    }

    @Provides
    @Singleton
    fun provideModePreferences(@ApplicationContext context: Context): ModePreferences {
        val prefs = context.getSharedPreferences("assistant_prefs", Context.MODE_PRIVATE)
        return object : ModePreferences {
            override fun getCurrentModeId(): String? {
                return prefs.getString("current_mode_id", null)
            }

            override fun setCurrentModeId(modeId: String?) {
                prefs.edit().apply {
                    if (modeId != null) {
                        putString("current_mode_id", modeId)
                    } else {
                        remove("current_mode_id")
                    }
                    apply()
                }
            }
        }
    }

    @Provides
    @Singleton
    fun provideModeManager(
        modeTree: ModeTree,
        modePreferences: ModePreferences
    ): ModeManager {
        return ModeManager(modeTree, modePreferences)
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
        logger: AssistantLogger,
        authErrorHandler: AuthErrorHandler,
        modeManager: ModeManager
    ): ChatRepository {
        return ChatRepositoryImpl(
            chatMessageDao = chatMessageDao,
            llmProvider = llmProvider,
            toolRegistry = toolRegistry,
            systemPromptBuilder = systemPromptBuilder,
            stringProvider = stringProvider,
            languagePreferences = languagePreferences,
            logger = logger,
            authErrorHandler = authErrorHandler,
            modeManager = modeManager
        )
    }
}
