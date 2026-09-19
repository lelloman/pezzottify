package com.lelloman.pezzottify.android.debuginterface

import com.lelloman.androidoscopy.Androidoscopy
import com.lelloman.androidoscopy.tools.Tool
import com.lelloman.androidoscopy.tools.ToolResult
import com.lelloman.pezzottify.android.domain.cache.CacheManager
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import com.lelloman.pezzottify.android.domain.catalogsync.CatalogSyncManager
import com.lelloman.pezzottify.android.domain.catalogsync.CatalogSyncStore
import com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository
import com.lelloman.pezzottify.android.domain.lifecycle.NetworkConnectivityObserver
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.*
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.sync.SyncManager
import com.lelloman.pezzottify.android.domain.sync.SyncState
import com.lelloman.pezzottify.android.domain.sync.SyncStateStore
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LogLevel
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.*
import javax.inject.Inject
import javax.inject.Provider
import javax.inject.Singleton

/** Explicit adapters to the live DI instances; no reflection, arbitrary SQL or private method calls. */
@Singleton
class PezzottifyDiagnosticTools @Inject constructor(
    private val player: Provider<PezzottifyPlayer>,
    private val playbackStore: Provider<PlaybackStateStore>,
    private val playbackMode: Provider<PlaybackModeManager>,
    private val cache: Provider<CacheManager>,
    private val staticsCache: Provider<StaticsCache>,
    private val sync: Provider<SyncManager>,
    private val syncStore: Provider<SyncStateStore>,
    private val catalogSync: Provider<CatalogSyncManager>,
    private val catalogStore: Provider<CatalogSyncStore>,
    private val webSocket: Provider<WebSocketManager>,
    private val network: Provider<NetworkConnectivityObserver>,
    private val statics: Provider<StaticsStore>,
    private val fetchStates: Provider<StaticItemFetchStateStore>,
    private val downloads: Provider<DownloadStatusRepository>,
    private val playlists: Provider<UserPlaylistStore>,
    private val listening: Provider<ListeningEventStore>,
    private val loggerFactory: LoggerFactory,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val mutations = Mutex()
    private var sessionObserver: Job? = null

    fun register() {
        loggerFactory.diagnosticLogs.configure {
            Androidoscopy.sessionState.value.let { if (it.active) it.sessionId else null }
        }
        sessionObserver?.cancel()
        sessionObserver = scope.launch {
            Androidoscopy.sessionState.collect { loggerFactory.diagnosticLogs.refreshSession() }
        }
        tools().forEach(Androidoscopy::registerTool)
    }

    internal fun tools(): List<Tool> = listOf(
        tool("playback", "Read live local player state, errors and a page of queue track IDs. State fields are a best-effort snapshot.", pageSchema) { args ->
            val p = player.get()
            val volume = p.volumeState.value
            val error = p.playerError.value
            obj(
                "active" to p.isActive.value, "playing" to p.isPlaying.value,
                "trackIndex" to p.currentTrackIndex.value, "positionSeconds" to p.currentTrackProgressSec.value,
                "durationSeconds" to p.currentTrackDurationSeconds.value,
                "volume" to volume.volume, "muted" to volume.isMuted,
                "shuffle" to p.shuffleEnabled.value, "repeat" to p.repeatMode.value.name,
                "mode" to when (val mode = playbackMode.get().mode.value) {
                    PlaybackMode.Local -> obj("type" to "local")
                    is PlaybackMode.Remote -> obj("type" to "remote", "deviceId" to mode.deviceId, "deviceName" to mode.deviceName)
                },
                "radioContinuationError" to p.radioContinuationError.value,
                "error" to error?.let { obj("trackId" to it.trackId, "message" to it.message, "code" to it.errorCode, "recoverable" to it.isRecoverable, "positionMs" to it.positionMs) },
                "queue" to p.playbackPlaylist.value?.let { queue(it, args) },
            )
        },
        tool("saved_playback", "Load persisted playback state and a page of its queue. WARNING: the store deletes expired/corrupt saved state when loading.", pageSchema, readOnly = false) { args ->
            obj("state" to playbackStore.get().loadState()?.let {
                obj("trackIndex" to it.currentTrackIndex, "positionMs" to it.positionMs, "playing" to it.isPlaying,
                    "savedAtMs" to it.savedAtMs, "queue" to queue(it.playlist, args))
            })
        },
        tool("cache_status", "Read memory/database/image cache sizes and per-type memory cache metrics.") {
            val stats = cache.get().getStats()
            obj("databaseBytes" to stats.staticsDatabaseSizeBytes, "memoryBytes" to stats.staticsMemorySizeBytes,
                "imageBytes" to stats.imageCacheSizeBytes,
                "memoryMetrics" to staticsCache.get().getAllMetrics().mapValues { (_, m) ->
                    obj("hits" to m.hits, "misses" to m.misses, "evictions" to m.evictions, "expirations" to m.expirations,
                        "entries" to m.currentEntries, "bytes" to m.currentSizeBytes, "hitRate" to m.hitRate)
                })
        },
        tool("sync_status", "Read user sync state, user/catalog cursors and full-sync requirement. Does not trigger network requests.") {
            val state = when (val s = sync.get().state.value) {
                SyncState.Idle -> obj("type" to "idle")
                SyncState.Syncing -> obj("type" to "syncing")
                is SyncState.Synced -> obj("type" to "synced", "cursor" to s.cursor)
                is SyncState.Error -> obj("type" to "error", "message" to s.message)
            }
            obj("state" to state, "userCursor" to syncStore.get().getCurrentCursor(),
                "catalogCursor" to catalogStore.get().currentSeq.value, "needsFullSync" to syncStore.get().needsFullSync())
        },
        tool("connectivity", "Read network availability and WebSocket state, including server version and connection errors.") {
            val connection = when (val c = webSocket.get().connectionState.value) {
                ConnectionState.Disconnected -> obj("type" to "disconnected")
                ConnectionState.Connecting -> obj("type" to "connecting")
                is ConnectionState.Connected -> obj("type" to "connected", "deviceId" to c.deviceId, "serverVersion" to c.serverVersion)
                is ConnectionState.Error -> obj("type" to "error", "message" to c.message)
            }
            obj("networkAvailable" to network.get().isNetworkAvailable.value, "webSocket" to connection)
        },
        tool("catalog_item", "Inspect a locally stored track, album or artist by ID, plus its fetch state. Does not fetch missing content. Lists are paginated.",
            schema(pageProperties + mapOf("id" to stringSchema(), "type" to enumSchema("track", "album", "artist")), "id", "type")) { args ->
            val id = args.string("id")
            val item = when (args.string("type")) {
                "track" -> statics.get().getTrack(id).first()?.let {
                    obj("id" to it.id, "name" to it.name, "albumId" to it.albumId,
                        "artists" to page(it.artistsIds, args), "durationSeconds" to it.durationSeconds, "availability" to it.availability.name)
                }
                "album" -> statics.get().getAlbum(id).first()?.let {
                    obj("id" to it.id, "name" to it.name, "date" to it.date, "imageId" to it.displayImageId,
                        "artists" to page(it.artistsIds, args), "tracks" to page(it.discs.flatMap { disc -> disc.tracksIds }, args),
                        "availability" to it.availability.name)
                }
                else -> statics.get().getArtist(id).first()?.let {
                    obj("id" to it.id, "name" to it.name, "imageId" to it.displayImageId, "related" to page(it.related, args))
                }
            }
            val fetch = fetchStates.get().get(id).first()
            obj("item" to item, "fetchState" to fetch?.let {
                obj("id" to it.itemId, "type" to it.itemType.name, "loading" to it.isLoading,
                    "error" to it.errorReason?.name, "lastAttemptMs" to it.lastAttemptTime, "nextAttemptMs" to it.tryNextTime)
            })
        },
        tool("download_status", "Read the currently known download request status for a content ID; null means unknown, not necessarily absent on the server.",
            schema(mapOf("id" to stringSchema()), "id")) { args ->
            obj("status" to downloads.get().observeStatus(args.string("id")).first()?.let { s ->
                obj("requestId" to s.requestId, "status" to s.status.name, "queuePosition" to s.queuePosition,
                    "error" to s.errorMessage, "createdAt" to s.createdAt, "progress" to s.progress?.let {
                        obj("total" to it.totalChildren, "completed" to it.completed, "failed" to it.failed, "pending" to it.pending, "inProgress" to it.inProgress)
                    })
            })
        },
        tool("playlist", "Read a local user playlist and a page of its track IDs without changing or syncing it.",
            schema(pageProperties + mapOf("id" to stringSchema()), "id")) { args ->
            obj("playlist" to playlists.get().getPlaylist(args.string("id")).first()?.let {
                obj("id" to it.id, "name" to it.name, "syncStatus" to it.syncStatus.name, "tracks" to page(it.trackIds, args))
            })
        },
        tool("sync_backlog", "Read a page of pending playlists and listening events, plus the number of loading catalog items. Includes listening history; does not upload it.", pageSchema) { args ->
            val pendingPlaylists = playlists.get().getPendingSyncPlaylists().first()
            val pendingEvents = listening.get().getPendingSyncEvents()
            obj("playlists" to page(pendingPlaylists, args) { obj("id" to it.id, "name" to it.name, "trackCount" to it.trackIds.size, "syncStatus" to it.syncStatus.name) },
                "listeningEvents" to page(pendingEvents, args) { obj("id" to it.id, "trackId" to it.trackId, "sessionId" to it.sessionId,
                    "startedAtMs" to it.startedAt, "endedAtMs" to it.endedAt, "durationSeconds" to it.durationSeconds,
                    "syncStatus" to it.syncStatus.name) },
                "loadingCatalogItems" to fetchStates.get().getLoadingItemsCount())
        },
        tool("logs", "Read UNREDACTED app logs (including Debug and stack traces) from this diagnostic session only. No logcat or historical files. Up to 1000 entries retained, 4096 characters per message; filter by exact tag/minimum level and poll using nextCursor. Never polls to keep the session alive automatically.",
            schema(mapOf("after" to obj("type" to "integer", "minimum" to 0, "maximum" to Long.MAX_VALUE), "limit" to integerSchema(1, 100),
                "minimumLevel" to enumSchema("Debug", "Info", "Warn", "Error"), "tag" to stringSchema()))) { args ->
            val logs = loggerFactory.diagnosticLogs.read(args["after"]?.jsonPrimitive?.long ?: 0,
                args.int("limit", 100), args["minimumLevel"]?.jsonPrimitive?.content?.let(LogLevel::valueOf) ?: LogLevel.Debug,
                args["tag"]?.jsonPrimitive?.content)
            obj("sessionId" to logs.sessionId, "nextCursor" to logs.nextCursor, "oldestAvailableId" to logs.oldestAvailableId,
                "hasMore" to logs.hasMore, "entries" to logs.entries.map {
                    obj("id" to it.id, "timestampMs" to it.timestampMs, "level" to it.level.name, "tag" to it.tag, "message" to it.message, "truncated" to it.truncated)
                })
        },
        tool("retry_playback", "Request retry of the current local playback error. Changes playback; completion does not imply recovery.", readOnly = false) {
            withContext(Dispatchers.Main.immediate) { player.get().retry() }
            obj("requested" to true)
        },
        tool("sync_catch_up", "Run user or catalog catch-up. Makes network requests and changes local state; user catch-up may fall back to full sync. Catalog completion does not guarantee success; check logs/cursor.",
            schema(mapOf("target" to enumSchema("user", "catalog")), "target"), readOnly = false) { args ->
            if (args.string("target") == "user") obj("success" to sync.get().catchUp())
            else { catalogSync.get().catchUp(); obj("completed" to true, "catalogCursor" to catalogStore.get().currentSeq.value) }
        },
        tool("reconnect", "Disconnect then reconnect the app's server WebSocket. Interrupts its connection and can trigger sync; completion is not proof of connection.", readOnly = false) {
            webSocket.get().disconnect()
            webSocket.get().connect()
            obj("requested" to true)
        },
        tool("cache_action", "Trim or clear statics (memory AND local database) or images. Removes cached content and may cause refetches; does not delete server content.",
            schema(mapOf("target" to enumSchema("statics", "images"), "action" to enumSchema("trim", "clear")), "target", "action"), readOnly = false) { args ->
            val manager = cache.get()
            when (args.string("target") to args.string("action")) {
                "statics" to "trim" -> manager.trimStaticsCache()
                "statics" to "clear" -> manager.clearStaticsCache()
                "images" to "trim" -> manager.trimImageCache()
                "images" to "clear" -> manager.clearImageCache()
            }
            obj("completed" to true)
        },
    )

    private fun tool(name: String, description: String, input: JsonObject = schema(emptyMap()), readOnly: Boolean = true,
                     handler: suspend (JsonObject) -> JsonObject) = Tool(
        name = "pezzottify_$name", description = description, inputSchema = input, readOnly = readOnly,
    ) { args ->
        withContext(Dispatchers.IO) {
            val result = if (readOnly) handler(args) else mutations.withLock { handler(args) }
            ToolResult.json(result).copy(isError = result["success"]?.jsonPrimitive?.booleanOrNull == false)
        }
    }

    private fun queue(playlist: PlaybackPlaylist, args: JsonObject) = obj(
        "context" to when (val c = playlist.context) {
            is PlaybackPlaylistContext.Album -> obj("type" to "album", "id" to c.albumId)
            is PlaybackPlaylistContext.UserPlaylist -> obj("type" to "playlist", "id" to c.userPlaylistId, "edited" to c.isEdited)
            PlaybackPlaylistContext.UserMix -> obj("type" to "mix")
            is PlaybackPlaylistContext.Radio -> obj("type" to "radio", "source" to c.source, "seedType" to c.seedEntityType,
                "seedId" to c.seedEntityId, "seedLabel" to c.seedLabel, "edited" to c.isEdited)
        },
        "tracks" to page(playlist.tracksIds, args), "hasContinuation" to (playlist.continuation != null),
    )

    companion object {
        private fun stringSchema() = obj("type" to "string", "minLength" to 1, "maxLength" to 512)
        private fun integerSchema(min: Int, max: Int) = obj("type" to "integer", "minimum" to min, "maximum" to max)
        private fun enumSchema(vararg values: String) = obj("type" to "string", "enum" to values.toList())
        private val pageProperties = mapOf("offset" to integerSchema(0, Int.MAX_VALUE), "limit" to integerSchema(1, 100))
        private val pageSchema = schema(pageProperties)
        private fun schema(properties: Map<String, JsonObject>, vararg required: String) = obj(
            "type" to "object", "properties" to properties, "additionalProperties" to false, "required" to required.toList(),
        )
        private fun JsonObject.string(key: String) = getValue(key).jsonPrimitive.content
        private fun JsonObject.int(key: String, default: Int) = get(key)?.jsonPrimitive?.int ?: default
        private fun <T> page(items: List<T>, args: JsonObject, transform: (T) -> Any? = { it }): JsonObject {
            val offset = args.int("offset", 0).coerceAtMost(items.size)
            val end = (offset.toLong() + args.int("limit", 100)).coerceAtMost(items.size.toLong()).toInt()
            return obj("total" to items.size, "offset" to offset, "nextOffset" to end.takeIf { it < items.size },
                "items" to items.subList(offset, end).map(transform))
        }
        private fun obj(vararg values: Pair<String, Any?>): JsonObject = JsonObject(values.associate { it.first to json(it.second) })
        private fun json(value: Any?): JsonElement = when (value) {
            null -> JsonNull
            is JsonElement -> value
            is String -> JsonPrimitive(value)
            is Boolean -> JsonPrimitive(value)
            is Number -> JsonPrimitive(value)
            is Map<*, *> -> JsonObject(value.entries.associate { it.key.toString() to json(it.value) })
            is Iterable<*> -> JsonArray(value.map(::json))
            else -> error("Unsupported diagnostic value type")
        }
    }
}
