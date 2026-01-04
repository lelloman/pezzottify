# Android AI Assistant

**Status**: Design Phase
**Created**: 2026-01-04

---

## Overview

AI chat assistant for the Android app, replicating the web's AI chat functionality with Android-native architecture.

**Goal**: Build a generic, reusable AI assistant module that can be shared across multiple apps (Pezzottify, SimpleEphem, etc.). Eventually this will become its own repository.

---

## Architecture

### Module Structure

The AI assistant is implemented as a **standalone generic module** with pluggable providers. App-specific tools are injected by the host app.

```
android/
│
├── simple-ai-assistant/              # Generic AI assistant module
│   ├── ui/
│   │   ├── ChatScreen.kt             # Full chat UI
│   │   ├── ChatMessageItem.kt        # Message bubble component
│   │   ├── ChatSettingsDialog.kt     # Provider/model configuration
│   │   └── ChatViewModel.kt          # UI state management
│   ├── data/
│   │   ├── ChatRepository.kt         # Message handling, tool execution
│   │   ├── ChatConfigStore.kt        # Settings persistence
│   │   └── ChatHistoryStore.kt       # Room DB for compacted context
│   ├── model/
│   │   ├── ChatMessage.kt            # Message data model
│   │   ├── ChatConfig.kt             # Configuration model
│   │   └── StreamEvent.kt            # LLM streaming events
│   ├── llm/
│   │   └── LlmProvider.kt            # Provider interface (NO implementations)
│   └── tool/
│       ├── Tool.kt                   # Tool interface
│       └── ToolExecutor.kt           # Tool orchestration
│
├── simple-ai-provider-ollama/        # Ollama provider (first implementation)
│   └── OllamaProvider.kt
│
├── simple-ai-provider-simpleai/      # SimpleAI provider (uses app auth)
│   └── SimpleAiProvider.kt
│
├── simple-ai-provider-openai/        # OpenAI provider (future)
│   └── OpenAiProvider.kt
│
├── simple-ai-provider-anthropic/     # Anthropic provider (future)
│   └── AnthropicProvider.kt
│
└── app/                              # Pezzottify app (host)
    └── di/
        └── AiAssistantModule.kt      # Wires provider + app-specific tools
```

### What's Generic (in `simple-ai-assistant`)

Lives in the shared module, knows nothing about Pezzottify:

- Chat UI (screen, messages, settings dialog)
- Message model & streaming logic
- Provider interface (`LlmProvider`)
- Tool interface (`Tool`)
- Tool executor (orchestrates tool calls)
- Config storage (preferences)
- History storage (Room DB)
- Context compaction logic
- Language detection

### What's Provider-Specific (in `simple-ai-provider-*`)

Each provider is a separate module implementing `LlmProvider`:

- HTTP client setup (OkHttp)
- SSE/streaming parsing
- Auth handling (API keys, tokens)
- Model listing
- Provider-specific configuration

### What's App-Specific (provided by host app)

The host app (Pezzottify) provides:

- Tool implementations (playback, playlists, navigation, etc.)
- Which provider module(s) to include at build time
- Provider configuration (API keys, URLs)
- Integration into app navigation (4th tab)

```kotlin
// In Pezzottify's app module
@Module
@InstallIn(SingletonComponent::class)
object AiAssistantModule {

    @Provides
    fun provideToolRegistry(
        player: PezzottifyPlayer,
        userContent: UserContentRepository,
        navigation: NavigationEventBus,
        // ...
    ): ToolRegistry = ToolRegistry(
        tools = listOf(
            PlayTool(player),
            PauseTool(player),
            CreatePlaylistTool(userContent),
            NavigateTool(navigation),
            // ... all Pezzottify-specific tools
        )
    )

    @Provides
    fun provideLlmProvider(
        ollamaProvider: OllamaProvider  // or SimpleAiProvider, etc.
    ): LlmProvider = ollamaProvider
}
```

### Key Principles

- `simple-ai-assistant` is completely generic and reusable
- Provider modules depend only on `simple-ai-assistant` (for the interface)
- Host app depends on `simple-ai-assistant` + chosen provider module(s)
- Host app injects app-specific tools via DI
- Eventually `simple-ai-assistant` + providers become a separate repository

---

## Decisions

### 1. UI Integration

**Decision**: AI as 4th tab in bottom navigation

- Tabs: Home | Search | Library | **AI**
- Full-screen chat experience
- Bottom player remains visible (not an overlay route)

**Rationale**: Discoverable, consistent with app patterns, gives chat dedicated space.

### 2. Message Persistence

**Decision**: Room DB is source of truth

- **All messages** stored in Room DB (1:1 with UI)
- ViewModel loads messages from Room on start
- New messages saved to Room immediately
- UI observes Room via Flow

**Compaction behavior**:
- When triggered, old messages are **deleted** from Room
- A summary message is **inserted** in their place
- UI automatically reflects the change (observing Room)

```
User sends message
    → Repository saves to Room
    → Room emits updated list via Flow
    → ViewModel receives update
    → UI displays new message
```

### 3. Tool Execution Model

**Decision**: Hybrid model based on user role

| User Role | Tools Available |
|-----------|-----------------|
| Regular | Android-native only (playback, playlists, likes, navigation) |
| Admin | Android-native + Server MCP (catalog, users, analytics, server control) |

#### Android-Native Tools (All Users)

All user-facing tools operate through the Android domain layer:

- Playback control (play, pause, queue, etc.)
- Playlist management (create, add tracks, delete)
- Likes (like/unlike albums, artists, tracks)
- Navigation (go to album, search, etc.)
- Settings

**Rationale**:
- Avoids "weird sync" where AI creates something server-side, then we get sync event
- AI becomes another "user" of existing domain layer
- State stays consistent
- Fast, local-first experience

**Example flow**:
```
AI decides to create playlist
  → calls domain CreatePlaylistUseCase
  → use case updates local DB + syncs to server
  → same path as user tapping "Create Playlist"
```

#### Server MCP Tools (Admin Only - Future)

For admin users, server MCP provides power tools. The "sync delay" is acceptable here since:
- Admin operations are less frequent
- User expects server roundtrip for admin tasks
- Some operations can only happen server-side

**Catalog Management**:
- "Add this new album to the catalog"
- "Fix the artist name on these tracks"
- "Merge these two duplicate artists"

**User Management**:
- "Show me all users and their roles"
- "Give user X permission to request downloads"
- "Who hasn't logged in for 30 days?"

**Analytics**:
- "What's the most played album this week?"
- "Show bandwidth usage by user"
- "Which tracks have never been played?"

**Server Operations**:
- "Run the integrity check job"
- "Show failed downloads and retry them"
- "What's the server status?"

**Download Manager**:
- "Queue this artist's discography for download"
- "Show pending download requests"
- "Approve/reject download candidates"

#### Implementation Plan

All of this is v1:
1. Android-native tools (all users)
2. Server MCP tools (admin users)

### 4. LLM Provider System

**Decision**: Plugin architecture with separate modules

- Interface defined in `simple-ai-assistant` module
- Each provider is a **separate Gradle module** (e.g., `simple-ai-provider-ollama`)
- Provider modules depend on `simple-ai-assistant` for the interface
- Host app includes desired provider module(s) at build time
- Provider-agnostic: app doesn't know which provider, just uses the interface

**First provider**: Ollama (local LLM, no API key needed, good for development)

**Future providers**:
- `simple-ai-provider-simpleai` - Uses app's existing auth token
- `simple-ai-provider-openai` - OpenAI API
- `simple-ai-provider-anthropic` - Claude API

```kotlin
// simple-ai-assistant/llm/LlmProvider.kt
interface LlmProvider {
    val id: String
    val displayName: String

    fun streamChat(
        messages: List<ChatMessage>,
        tools: List<ToolSpec>,
        systemPrompt: String
    ): Flow<StreamEvent>

    suspend fun testConnection(): Result<Unit>
    suspend fun listModels(): Result<List<String>>
}
```

### 5. Configuration & Settings

| Setting | Storage | Notes |
|---------|---------|-------|
| Provider selection | SharedPreferences | Which provider module to use |
| API keys | Provider-specific | Each module handles its own (EncryptedSharedPrefs) |
| Model selection | SharedPreferences | Per-provider model choice |
| Base URL (Ollama) | SharedPreferences | For self-hosted instances |
| Language preference | SharedPreferences | Auto-detect or explicit |
| Debug mode | SharedPreferences | Show technical details |

**Language detection**: Same as web - LLM-based detection on first message, persisted preference.

### 6. Offline Behavior

**Decision**: No network = show error

- No offline queue
- No caching of unsent messages
- Simple error state in UI

### 7. Error Handling

*To be defined*

### 8. Platform-Specific Considerations

*Out of scope for v1*

---

## Data Models

```kotlin
data class ChatMessage(
    val id: String,
    val role: MessageRole,
    val content: String,
    val toolCalls: List<ToolCall>? = null,
    val toolCallId: String? = null,
    val toolName: String? = null,
    val timestamp: Long = System.currentTimeMillis()
)

enum class MessageRole { USER, ASSISTANT, TOOL }

data class ToolCall(
    val id: String,
    val name: String,
    val input: Map<String, Any?>
)

sealed class StreamEvent {
    data class Text(val content: String) : StreamEvent()
    data class ToolUse(val id: String, val name: String, val input: Map<String, Any?>) : StreamEvent()
    data class Error(val message: String) : StreamEvent()
    object Done : StreamEvent()
}
```

---

## Tools

### Philosophy

Tools call existing Android domain layer components. The AI operates like another user of the app.

### Categories

#### Playback Tools
*Uses existing `PezzottifyPlayer` interface*

| Tool | Description | Domain Component |
|------|-------------|------------------|
| `play` | Play/resume, optionally specific track | `Player.play()` |
| `pause` | Pause playback | `Player.pause()` |
| `playPause` | Toggle play/pause | `Player.playPause()` |
| `next` | Skip to next track | `Player.next()` |
| `previous` | Go to previous track | `Player.previous()` |
| `queue` | Add tracks to queue | `Player.addToQueue()` |
| `getCurrentTrack` | Get current playback state | `Player.currentTrack` |
| `setVolume` | Set volume (0.0-1.0) | `Player.setVolume()` |
| `playAlbum` | Play entire album | `Player.loadAlbum()` |
| `playPlaylist` | Play user playlist | `Player.loadUserPlaylist()` |

#### Navigation Tools
*Uses navigation event bus or similar*

| Tool | Description |
|------|-------------|
| `navigate` | Go to album/artist/track/playlist/settings/home |
| `search` | Navigate to search with query |

#### Content Tools
*Uses existing repositories*

| Tool | Description | Domain Component |
|------|-------------|------------------|
| `likeAlbum` | Like an album | `ToggleLikeUseCase` |
| `unlikeAlbum` | Unlike an album | `ToggleLikeUseCase` |
| `likeArtist` | Like an artist | `ToggleLikeUseCase` |
| `unlikeArtist` | Unlike an artist | `ToggleLikeUseCase` |
| `getLikedContent` | Get all liked content | `UserContentRepository` |

#### Playlist Tools
*Uses existing playlist management*

| Tool | Description | Domain Component |
|------|-------------|------------------|
| `getPlaylists` | List user playlists | `UserContentRepository` |
| `createPlaylist` | Create new playlist | `CreatePlaylistUseCase` |
| `addToPlaylist` | Add tracks to playlist | `AddToPlaylistUseCase` |
| `deletePlaylist` | Delete playlist | `DeletePlaylistUseCase` |

#### Search/Query Tools
*Uses existing search functionality*

| Tool | Description | Domain Component |
|------|-------------|------------------|
| `searchCatalog` | Search artists/albums/tracks | `PerformSearch` use case |
| `getAlbum` | Get album details with tracks | `StaticsRepository` |
| `getArtist` | Get artist details | `StaticsRepository` |
| `getTrack` | Get track details | `StaticsRepository` |

#### Settings Tools

| Tool | Description |
|------|-------------|
| `getSetting` | Get a setting value |
| `setSetting` | Update a setting |

#### Meta Tools

| Tool | Description |
|------|-------------|
| `help` | Get documentation by category |

---

## UI Components

### ChatScreen

Full-screen composable as 4th tab:

```
┌─────────────────────────────┐
│  AI Assistant        ⚙️ 🗑️  │  <- Header with settings, clear
├─────────────────────────────┤
│                             │
│  [User message]             │
│                             │
│        [Assistant message]  │
│        🔧 Played album...   │
│                             │
│  [User message]             │
│                             │
│        [Assistant typing_]  │  <- Streaming indicator
│                             │
├─────────────────────────────┤
│  [Message input...    ] ➤   │  <- Text field + send
├─────────────────────────────┤
│  🏠   🔍   📚   🤖          │  <- Bottom nav (AI selected)
└─────────────────────────────┘
```

### ChatSettingsDialog

Modal dialog for configuration:

- Provider dropdown
- API key input (if needed)
- Model selection
- Base URL (for Ollama)
- Language picker
- Debug mode toggle
- Test connection button

### ChatMessageItem

Message bubble with:

- User messages: Right-aligned, accent color
- Assistant messages: Left-aligned, surface color
- Tool indicators: Friendly description or debug JSON
- Streaming: Blinking cursor animation

---

## Context Compaction

Same algorithm as web:

1. **Hot zone**: Last ~4,000 tokens kept verbatim
2. **Staging area**: Older messages exceeding threshold
3. When staging exceeds limit, use LLM to create summary
4. Summary preserves: task outcomes, user preferences, important decisions
5. Runs async after each response

**Token estimation**: ~3.5 chars = 1 token

**Persistence**: Compacted summary stored in Room DB, loaded on app restart.

---

## Implementation Phases

### Phase 1: Core Module Setup (`simple-ai-assistant`)
- [ ] Create `simple-ai-assistant` Gradle module
- [ ] Data models (`ChatMessage`, `ChatConfig`, `StreamEvent`)
- [ ] `LlmProvider` interface
- [ ] `Tool` interface
- [ ] `ToolExecutor` for tool orchestration
- [ ] `ChatRepository` (message handling, tool execution)
- [ ] `ChatConfigStore` (SharedPreferences)
- [ ] `ChatHistoryStore` (Room DB for compacted context)

### Phase 2: UI (`simple-ai-assistant`)
- [ ] `ChatScreen` composable
- [ ] `ChatViewModel`
- [ ] `ChatMessageItem` composable (user/assistant/tool messages)
- [ ] `ChatSettingsDialog`
- [ ] Streaming text display with cursor animation
- [ ] Language picker
- [ ] Debug mode toggle

### Phase 3: First Provider (`simple-ai-provider-ollama`)
- [ ] Create `simple-ai-provider-ollama` Gradle module
- [ ] `OllamaProvider` implementing `LlmProvider`
- [ ] SSE streaming with OkHttp
- [ ] Model listing from `/api/tags`
- [ ] Connection testing

### Phase 4: Pezzottify Integration
- [ ] Add AI tab to bottom navigation
- [ ] `AiAssistantModule` in app (DI wiring)
- [ ] Android-native tools (all users):
  - [ ] Playback tools (10)
  - [ ] Navigation tools (2)
  - [ ] Content tools (5)
  - [ ] Playlist tools (4)
  - [ ] Search/Query tools (4)
  - [ ] Settings tools (2)
  - [ ] Meta tools (1)
- [ ] Server MCP tools (admin users):
  - [ ] MCP WebSocket client
  - [ ] Catalog management tools
  - [ ] User management tools
  - [ ] Analytics tools
  - [ ] Server operations tools
  - [ ] Download manager tools

### Phase 5: Polish
- [ ] Context compaction
- [ ] Language detection
- [ ] Error handling
- [ ] Loading states

### Future
- [ ] Additional providers (`simple-ai-provider-simpleai`, `openai`, `anthropic`)
- [ ] Extract to separate repository

---

## Open Questions

1. **Tool refinement**: Review each tool's exact parameters and return types
2. **Error handling**: Define error states and recovery flows
3. **Future providers**: SimpleAI with app auth token (uses what backend?)

---

## References

- Web implementation: `web/src/components/chat/`, `web/src/store/chat.js`, `web/src/services/llm/`
- Web tools: `web/src/services/uiTools.js`, `web/src/services/mcp.js`
