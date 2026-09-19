package com.lelloman.pezzottify.android.debuginterface

import com.lelloman.pezzottify.android.domain.cache.*
import com.lelloman.pezzottify.android.domain.catalogsync.*
import com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository
import com.lelloman.pezzottify.android.domain.lifecycle.NetworkConnectivityObserver
import com.lelloman.pezzottify.android.domain.listening.ListeningEventStore
import com.lelloman.pezzottify.android.domain.player.*
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.fetchstate.StaticItemFetchStateStore
import com.lelloman.pezzottify.android.domain.sync.*
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.domain.websocket.*
import com.lelloman.pezzottify.android.logger.*
import io.mockk.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.test.*
import kotlinx.serialization.json.*
import org.junit.Assert.*
import org.junit.Test
import javax.inject.Provider

@OptIn(ExperimentalCoroutinesApi::class)
class PezzottifyDiagnosticToolsTest {
    private val player = mockk<PezzottifyPlayer>(relaxed = true)
    private val playbackStore = mockk<PlaybackStateStore>()
    private val mode = mockk<PlaybackModeManager>()
    private val cache = mockk<CacheManager>(relaxed = true)
    private val memory = mockk<StaticsCache>()
    private val sync = mockk<SyncManager>()
    private val syncStore = mockk<SyncStateStore>()
    private val catalogSync = mockk<CatalogSyncManager>()
    private val catalogStore = mockk<CatalogSyncStore>()
    private val socket = mockk<WebSocketManager>(relaxed = true)
    private val network = mockk<NetworkConnectivityObserver>()
    private val statics = mockk<StaticsStore>()
    private val fetchStates = mockk<StaticItemFetchStateStore>()
    private val downloads = mockk<DownloadStatusRepository>()
    private val playlists = mockk<UserPlaylistStore>()
    private val listening = mockk<ListeningEventStore>()
    private val logger = LoggerFactory(MutableStateFlow(LogLevel.None))
    private val subject = PezzottifyDiagnosticTools(
        Provider { player }, Provider { playbackStore }, Provider { mode }, Provider { cache }, Provider { memory },
        Provider { sync }, Provider { syncStore }, Provider { catalogSync }, Provider { catalogStore },
        Provider { socket }, Provider { network }, Provider { statics }, Provider { fetchStates },
        Provider { downloads }, Provider { playlists }, Provider { listening }, logger,
    )
    private val tools by lazy { subject.tools().associateBy { it.name.removePrefix("pezzottify_") } }
    private suspend fun call(name: String, args: String = "{}"): JsonObject {
        val tool = tools.getValue(name)
        val input = Json.parseToJsonElement(args).jsonObject
        tool.validateInput(input)
        return tool.handler(input).structuredContent!!
    }

    @Test fun `manifest is explicit lazy and marks all mutations`() {
        assertEquals(14, tools.size)
        assertEquals(setOf("saved_playback", "retry_playback", "sync_catch_up", "reconnect", "cache_action"),
            tools.filterValues { !it.readOnly }.keys)
        verify { listOf(player, playbackStore, mode, cache, memory, sync, syncStore, catalogSync, catalogStore,
            socket, network, statics, fetchStates, downloads, playlists, listening) wasNot Called }
    }

    @Test fun `schemas reject unknown fields invalid targets and oversized reads`() {
        listOf("logs" to "{\"limit\":101}", "logs" to "{\"after\":-1}", "playback" to "{\"offset\":-1}",
            "cache_action" to "{\"target\":\"all\",\"action\":\"clear\"}", "catalog_item" to "{}",
            "reconnect" to "{\"anything\":true}").forEach { (name, args) ->
            assertThrows(IllegalArgumentException::class.java) { tools.getValue(name).validateInput(Json.parseToJsonElement(args).jsonObject) }
        }
    }

    @Test fun `saved queue is paginated including offsets beyond end`() = runBlocking {
        coEvery { playbackStore.loadState() } returns SavedPlaybackState(
            PlaybackPlaylist(PlaybackPlaylistContext.UserMix, listOf("a", "b", "c")), 1, 1200, false, 123,
        )
        val tracks = call("saved_playback", "{\"offset\":1,\"limit\":1}")["state"]!!.jsonObject["queue"]!!.jsonObject["tracks"]!!.jsonObject
        assertEquals(3, tracks["total"]!!.jsonPrimitive.int)
        assertEquals("b", tracks["items"]!!.jsonArray.single().jsonPrimitive.content)
        assertEquals(2, tracks["nextOffset"]!!.jsonPrimitive.int)
        val empty = call("saved_playback", "{\"offset\":2147483647}")["state"]!!.jsonObject["queue"]!!.jsonObject["tracks"]!!.jsonObject
        assertTrue(empty["items"]!!.jsonArray.isEmpty())
        coVerify(exactly = 0) { playbackStore.clearState() }
    }

    @Test fun `connectivity and sync reads never connect or sync`() = runBlocking {
        every { socket.connectionState } returns MutableStateFlow(ConnectionState.Connected(7, "v1"))
        every { network.isNetworkAvailable } returns MutableStateFlow(true)
        every { sync.state } returns MutableStateFlow(SyncState.Error("raw failure"))
        every { syncStore.getCurrentCursor() } returns 12
        every { syncStore.needsFullSync() } returns true
        every { catalogStore.currentSeq } returns MutableStateFlow(34)
        assertEquals("v1", call("connectivity")["webSocket"]!!.jsonObject["serverVersion"]!!.jsonPrimitive.content)
        assertEquals(12, call("sync_status")["userCursor"]!!.jsonPrimitive.int)
        coVerify(exactly = 0) { socket.connect(); socket.disconnect(); sync.catchUp(); sync.fullSync() }
    }

    @Test fun `cache action only calls selected target`() = runBlocking {
        call("cache_action", "{\"target\":\"images\",\"action\":\"clear\"}")
        coVerify(exactly = 1) { cache.clearImageCache() }
        coVerify(exactly = 0) { cache.clearStaticsCache(); cache.trimStaticsCache(); cache.trimImageCache() }
    }

    @Test fun `cache status returns metrics without clearing anything`() = runBlocking {
        coEvery { cache.getStats() } returns CacheStats(10, 20, 30)
        every { memory.getAllMetrics() } returns mapOf("track" to CacheMetrics(4, 1, 2, 3, 5, 20, 0.8))
        val result = call("cache_status")
        assertEquals(10, result["databaseBytes"]!!.jsonPrimitive.int)
        assertEquals(4, result["memoryMetrics"]!!.jsonObject["track"]!!.jsonObject["hits"]!!.jsonPrimitive.int)
        coVerify(exactly = 0) { cache.clearStaticsCache(); cache.clearImageCache() }
        verify(exactly = 0) { memory.clearAll() }
    }

    @Test fun `missing local entities return null without writes`() = runBlocking {
        every { statics.getTrack("missing") } returns flowOf(null)
        every { fetchStates.get("missing") } returns flowOf(null)
        every { downloads.observeStatus("missing") } returns flowOf(null)
        every { playlists.getPlaylist("missing") } returns flowOf(null)
        assertEquals(JsonNull, call("catalog_item", "{\"id\":\"missing\",\"type\":\"track\"}")["item"])
        assertEquals(JsonNull, call("download_status", "{\"id\":\"missing\"}")["status"])
        assertEquals(JsonNull, call("playlist", "{\"id\":\"missing\"}")["playlist"])
        coVerify(exactly = 0) { statics.deleteAll(); downloads.clear(); playlists.deleteAll() }
    }

    @Test fun `backlog exposes counts without uploading events`() = runBlocking {
        every { playlists.getPendingSyncPlaylists() } returns flowOf(emptyList())
        coEvery { listening.getPendingSyncEvents() } returns emptyList()
        coEvery { fetchStates.getLoadingItemsCount() } returns 3
        val result = call("sync_backlog")
        assertEquals(3, result["loadingCatalogItems"]!!.jsonPrimitive.int)
        assertEquals(0, result["listeningEvents"]!!.jsonObject["total"]!!.jsonPrimitive.int)
        coVerify(exactly = 0) { listening.deleteAll(); sync.catchUp() }
    }

    @Test fun `reconnect disconnects first and errors propagate`() = runBlocking {
        call("reconnect")
        coVerifyOrder { socket.disconnect(); socket.connect() }
        coEvery { socket.disconnect() } throws IllegalStateException("failure")
        try { call("reconnect"); fail("Expected failure") } catch (e: IllegalStateException) { assertEquals("failure", e.message) }
        coVerify(exactly = 1) { socket.connect() }
    }

    @Test fun `retry dispatches to main`() = runTest {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        try {
            call("retry_playback")
            verify(exactly = 1) { player.retry() }
        } finally { Dispatchers.resetMain() }
    }

    @Test fun `sync catch up reports failures and propagates cancellation`() = runBlocking {
        coEvery { sync.catchUp() } returns false
        assertFalse(call("sync_catch_up", "{\"target\":\"user\"}")["success"]!!.jsonPrimitive.boolean)
        coEvery { sync.catchUp() } throws CancellationException("stopped")
        try { call("sync_catch_up", "{\"target\":\"user\"}"); fail("Expected cancellation") } catch (_: CancellationException) { }
        coVerify(exactly = 0) { catalogSync.catchUp() }
    }

    @Test fun `log tool exposes raw messages only for the current session`() = runBlocking {
        var session: String? = "session"
        logger.diagnosticLogs.configure { session }
        logger.diagnosticLogs.record(LogLevel.Debug, "auth", "token=raw-secret")
        val result = call("logs")
        assertEquals("token=raw-secret", result["entries"]!!.jsonArray.single().jsonObject["message"]!!.jsonPrimitive.content)
        session = null
        assertTrue(call("logs")["entries"]!!.jsonArray.isEmpty())
    }

    @Test fun `large escaped logs fit a transport frame and can be drained without gaps`() = runBlocking {
        logger.diagnosticLogs.configure { "session" }
        repeat(100) { logger.diagnosticLogs.record(LogLevel.Error, "tag", "\u0001".repeat(4096)) }
        val tool = tools.getValue("logs")
        var cursor = 0L
        var count = 0
        do {
            val result = tool.handler(buildJsonObject { put("after", cursor) })
            assertTrue(result.toJson().toString().toByteArray().size < 1024 * 1024 - 1024)
            val data = result.structuredContent!!
            count += data["entries"]!!.jsonArray.size
            cursor = data["nextCursor"]!!.jsonPrimitive.long
        } while (data["hasMore"]!!.jsonPrimitive.boolean)
        assertEquals(100, count)
    }
}
