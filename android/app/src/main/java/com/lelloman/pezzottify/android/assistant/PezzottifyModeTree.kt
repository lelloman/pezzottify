package com.lelloman.pezzottify.android.assistant

import com.lelloman.simpleaiassistant.mode.AssistantMode
import com.lelloman.simpleaiassistant.mode.ModeTree

/**
 * Defines the mode hierarchy for the Pezzottify AI assistant.
 *
 * Modes:
 * - General: All tools available (root mode, good for general questions)
 * - Catalog: Focus on music discovery and search
 * - Playback: Focus on controlling playback and managing queue
 * - Playlists: Focus on playlist management
 * - Help: For app help and guidance (no tools, just conversation)
 */
object PezzottifyModeTree {

    // Tool IDs - must match the tool names in ToolRegistry
    private object Tools {
        // Search/Catalog tools
        const val SEARCH_CATALOG = "search_catalog"
        const val GET_ARTIST_DISCOGRAPHY = "get_artist_discography"
        const val WHATS_NEW = "whats_new"

        // Playback control tools
        const val PLAYBACK_CONTROL = "playback_control"
        const val SKIP_TRACK = "skip_track"
        const val PLAYBACK_MODE = "playback_mode"
        const val NOW_PLAYING = "now_playing"
        const val QUEUE = "queue"
        const val PLAY_ALBUM = "play_album"

        // Playlist tools
        const val LIST_PLAYLISTS = "list_playlists"
        const val VIEW_PLAYLIST = "view_playlist"
        const val CREATE_PLAYLIST = "create_playlist"
        const val RENAME_PLAYLIST = "rename_playlist"
        const val DELETE_PLAYLIST = "delete_playlist"
        const val ADD_TRACKS_TO_PLAYLIST = "add_tracks_to_playlist"
        const val REMOVE_TRACKS_FROM_PLAYLIST = "remove_tracks_from_playlist"
        const val PLAY_PLAYLIST = "play_playlist"
        const val ADD_PLAYLIST_TO_QUEUE = "add_playlist_to_queue"
    }

    // Mode IDs - stable identifiers for modes
    object ModeIds {
        const val GENERAL = "general"
        const val CATALOG = "catalog"
        const val PLAYBACK = "playback"
        const val PLAYLISTS = "playlists"
        const val HELP = "help"
    }

    private val catalogTools = setOf(
        Tools.SEARCH_CATALOG,
        Tools.GET_ARTIST_DISCOGRAPHY,
        Tools.WHATS_NEW
    )

    private val playbackTools = setOf(
        Tools.PLAYBACK_CONTROL,
        Tools.SKIP_TRACK,
        Tools.PLAYBACK_MODE,
        Tools.NOW_PLAYING,
        Tools.QUEUE,
        Tools.PLAY_ALBUM
    )

    private val playlistTools = setOf(
        Tools.LIST_PLAYLISTS,
        Tools.VIEW_PLAYLIST,
        Tools.CREATE_PLAYLIST,
        Tools.RENAME_PLAYLIST,
        Tools.DELETE_PLAYLIST,
        Tools.ADD_TRACKS_TO_PLAYLIST,
        Tools.REMOVE_TRACKS_FROM_PLAYLIST,
        Tools.PLAY_PLAYLIST,
        Tools.ADD_PLAYLIST_TO_QUEUE
    )

    private val allTools = catalogTools + playbackTools + playlistTools

    /**
     * Creates the mode tree for Pezzottify.
     */
    fun create(): ModeTree {
        val generalMode = AssistantMode(
            id = ModeIds.GENERAL,
            name = "General",
            description = "General assistant with access to all music features",
            toolIds = allTools,
            promptInstructions = """
                You have access to all tools. Help the user with any music-related task:
                - Search and discover music
                - Control playback
                - Manage playlists

                If the user focuses on a specific area (e.g., only playback control, only playlist management),
                proactively call the switch_mode tool to change to the appropriate specialized mode.
                Do NOT just mention the mode in text - actually call the tool to switch.
            """.trimIndent()
        )

        val catalogMode = AssistantMode(
            id = ModeIds.CATALOG,
            name = "Music Discovery",
            description = "Search and explore the music catalog",
            toolIds = catalogTools + setOf(Tools.PLAY_ALBUM), // Can play what they find
            promptInstructions = """
                Focus on helping the user discover music:
                - Search for artists, albums, and tracks
                - Explore artist discographies
                - Show what's new in the catalog
                - Help find specific songs or albums

                When the user finds something they like, you can play it with play_album.
                If they need more playback control features, call switch_mode to change to Playback mode.
            """.trimIndent()
        )

        val playbackMode = AssistantMode(
            id = ModeIds.PLAYBACK,
            name = "Playback Control",
            description = "Control music playback and queue",
            toolIds = playbackTools + setOf(Tools.SEARCH_CATALOG), // Can search to add tracks
            promptInstructions = """
                Focus on playback control:
                - Play, pause, skip tracks
                - Manage the playback queue
                - Control shuffle and repeat modes
                - Show what's currently playing

                You can search for music to add to the queue.
                If the user wants to manage playlists, call switch_mode to change to Playlists mode.
            """.trimIndent()
        )

        val playlistsMode = AssistantMode(
            id = ModeIds.PLAYLISTS,
            name = "Playlist Management",
            description = "Create and manage playlists",
            toolIds = playlistTools + setOf(Tools.SEARCH_CATALOG), // Can search to add tracks
            promptInstructions = """
                Focus on playlist management:
                - View and list playlists
                - Create new playlists
                - Add or remove tracks from playlists
                - Rename or delete playlists
                - Play playlists or add them to the queue

                IMPORTANT: When searching for tracks to add to playlists, NEVER use
                include_unavailable=true. Playlists should only contain playable tracks.
                The default search behavior already excludes unavailable content.

                If the user wants more playback control, call switch_mode to change to Playback mode.
            """.trimIndent()
        )

        val helpMode = AssistantMode(
            id = ModeIds.HELP,
            name = "App Help",
            description = "Get help with using the Pezzottify app",
            toolIds = emptySet(), // No tools needed, just conversation
            promptInstructions = """
                Help the user understand how to use Pezzottify:
                - Explain app features and how to use them
                - Describe the different modes and their purpose
                - Answer questions about music playback, search, and playlists
                - Provide tips for getting the most out of the app

                Be friendly and patient. If the user wants to do something specific,
                call switch_mode to change to the appropriate mode (General, Catalog, Playback, or Playlists).
            """.trimIndent()
        )

        // Build tree with General as root and others as children
        val rootMode = generalMode.copy(
            children = listOf(catalogMode, playbackMode, playlistsMode, helpMode)
        )

        return ModeTree(rootMode)
    }
}
