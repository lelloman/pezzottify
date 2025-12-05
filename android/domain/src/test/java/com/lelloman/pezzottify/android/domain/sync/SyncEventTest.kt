package com.lelloman.pezzottify.android.domain.sync

import com.google.common.truth.Truth.assertThat
import kotlinx.serialization.json.Json
import org.junit.Test

class SyncEventTest {

    private val json = Json {
        ignoreUnknownKeys = true
    }

    // region StoredEvent deserialization

    @Test
    fun `deserialize content_liked event from JSON`() {
        val jsonString = """
            {
                "seq": 42,
                "type": "content_liked",
                "payload": {
                    "content_type": "album",
                    "content_id": "album_123"
                },
                "server_timestamp": 1701700000
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)

        assertThat(storedEvent.seq).isEqualTo(42L)
        assertThat(storedEvent.type).isEqualTo("content_liked")
        assertThat(storedEvent.serverTimestamp).isEqualTo(1701700000L)
        assertThat(storedEvent.payload.contentType).isEqualTo(LikedContentType.Album)
        assertThat(storedEvent.payload.contentId).isEqualTo("album_123")

        val event = storedEvent.toSyncEvent()
        assertThat(event).isInstanceOf(SyncEvent.ContentLiked::class.java)
        val contentLiked = event as SyncEvent.ContentLiked
        assertThat(contentLiked.contentType).isEqualTo(LikedContentType.Album)
        assertThat(contentLiked.contentId).isEqualTo("album_123")
    }

    @Test
    fun `deserialize content_unliked event from JSON`() {
        val jsonString = """
            {
                "seq": 43,
                "type": "content_unliked",
                "payload": {
                    "content_type": "track",
                    "content_id": "track_456"
                },
                "server_timestamp": 1701700001
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)

        assertThat(storedEvent.type).isEqualTo("content_unliked")
        val event = storedEvent.toSyncEvent()
        assertThat(event).isInstanceOf(SyncEvent.ContentUnliked::class.java)
        val contentUnliked = event as SyncEvent.ContentUnliked
        assertThat(contentUnliked.contentType).isEqualTo(LikedContentType.Track)
        assertThat(contentUnliked.contentId).isEqualTo("track_456")
    }

    @Test
    fun `deserialize playlist_created event from JSON`() {
        val jsonString = """
            {
                "seq": 44,
                "type": "playlist_created",
                "payload": {
                    "playlist_id": "playlist_789",
                    "name": "My Playlist"
                },
                "server_timestamp": 1701700002
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)
        val event = storedEvent.toSyncEvent()

        assertThat(event).isInstanceOf(SyncEvent.PlaylistCreated::class.java)
        val playlistCreated = event as SyncEvent.PlaylistCreated
        assertThat(playlistCreated.playlistId).isEqualTo("playlist_789")
        assertThat(playlistCreated.name).isEqualTo("My Playlist")
    }

    @Test
    fun `deserialize playlist_renamed event from JSON`() {
        val jsonString = """
            {
                "seq": 45,
                "type": "playlist_renamed",
                "payload": {
                    "playlist_id": "playlist_789",
                    "name": "Renamed Playlist"
                },
                "server_timestamp": 1701700003
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)
        val event = storedEvent.toSyncEvent()

        assertThat(event).isInstanceOf(SyncEvent.PlaylistRenamed::class.java)
        val playlistRenamed = event as SyncEvent.PlaylistRenamed
        assertThat(playlistRenamed.playlistId).isEqualTo("playlist_789")
        assertThat(playlistRenamed.name).isEqualTo("Renamed Playlist")
    }

    @Test
    fun `deserialize playlist_deleted event from JSON`() {
        val jsonString = """
            {
                "seq": 46,
                "type": "playlist_deleted",
                "payload": {
                    "playlist_id": "playlist_789"
                },
                "server_timestamp": 1701700004
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)
        val event = storedEvent.toSyncEvent()

        assertThat(event).isInstanceOf(SyncEvent.PlaylistDeleted::class.java)
        val playlistDeleted = event as SyncEvent.PlaylistDeleted
        assertThat(playlistDeleted.playlistId).isEqualTo("playlist_789")
    }

    @Test
    fun `deserialize playlist_tracks_updated event from JSON`() {
        val jsonString = """
            {
                "seq": 47,
                "type": "playlist_tracks_updated",
                "payload": {
                    "playlist_id": "playlist_789",
                    "track_ids": ["track_1", "track_2", "track_3"]
                },
                "server_timestamp": 1701700005
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)
        val event = storedEvent.toSyncEvent()

        assertThat(event).isInstanceOf(SyncEvent.PlaylistTracksUpdated::class.java)
        val playlistTracks = event as SyncEvent.PlaylistTracksUpdated
        assertThat(playlistTracks.playlistId).isEqualTo("playlist_789")
        assertThat(playlistTracks.trackIds).containsExactly("track_1", "track_2", "track_3")
    }

    @Test
    fun `deserialize permission_granted event from JSON`() {
        val jsonString = """
            {
                "seq": 48,
                "type": "permission_granted",
                "payload": {
                    "permission": "EditCatalog"
                },
                "server_timestamp": 1701700006
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)
        val event = storedEvent.toSyncEvent()

        assertThat(event).isInstanceOf(SyncEvent.PermissionGranted::class.java)
        val permissionGranted = event as SyncEvent.PermissionGranted
        assertThat(permissionGranted.permission).isEqualTo(Permission.EditCatalog)
    }

    @Test
    fun `deserialize permission_revoked event from JSON`() {
        val jsonString = """
            {
                "seq": 49,
                "type": "permission_revoked",
                "payload": {
                    "permission": "LikeContent"
                },
                "server_timestamp": 1701700007
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)
        val event = storedEvent.toSyncEvent()

        assertThat(event).isInstanceOf(SyncEvent.PermissionRevoked::class.java)
        val permissionRevoked = event as SyncEvent.PermissionRevoked
        assertThat(permissionRevoked.permission).isEqualTo(Permission.LikeContent)
    }

    @Test
    fun `deserialize permissions_reset event from JSON`() {
        val jsonString = """
            {
                "seq": 50,
                "type": "permissions_reset",
                "payload": {
                    "permissions": ["AccessCatalog", "LikeContent", "OwnPlaylists"]
                },
                "server_timestamp": 1701700008
            }
        """.trimIndent()

        val storedEvent = json.decodeFromString<StoredEvent>(jsonString)
        val event = storedEvent.toSyncEvent()

        assertThat(event).isInstanceOf(SyncEvent.PermissionsReset::class.java)
        val permissionsReset = event as SyncEvent.PermissionsReset
        assertThat(permissionsReset.permissions).containsExactly(
            Permission.AccessCatalog,
            Permission.LikeContent,
            Permission.OwnPlaylists,
        )
    }

    // endregion

    // region LikedContentType serialization

    @Test
    fun `LikedContentType Album serializes to 'album'`() {
        val serialized = json.encodeToString(LikedContentType.serializer(), LikedContentType.Album)
        assertThat(serialized).isEqualTo("\"album\"")
    }

    @Test
    fun `LikedContentType Artist serializes to 'artist'`() {
        val serialized = json.encodeToString(LikedContentType.serializer(), LikedContentType.Artist)
        assertThat(serialized).isEqualTo("\"artist\"")
    }

    @Test
    fun `LikedContentType Track serializes to 'track'`() {
        val serialized = json.encodeToString(LikedContentType.serializer(), LikedContentType.Track)
        assertThat(serialized).isEqualTo("\"track\"")
    }

    @Test
    fun `LikedContentType deserializes from lowercase strings`() {
        assertThat(json.decodeFromString<LikedContentType>("\"album\"")).isEqualTo(LikedContentType.Album)
        assertThat(json.decodeFromString<LikedContentType>("\"artist\"")).isEqualTo(LikedContentType.Artist)
        assertThat(json.decodeFromString<LikedContentType>("\"track\"")).isEqualTo(LikedContentType.Track)
    }

    // endregion

    // region Unknown event handling

    @Test
    fun `toSyncEvent returns null for unknown event type`() {
        val storedEvent = StoredEvent(
            seq = 100,
            type = "unknown_event",
            payload = SyncEventPayload(),
            serverTimestamp = 1701700000,
        )

        val event = storedEvent.toSyncEvent()

        assertThat(event).isNull()
    }

    @Test
    fun `toSyncEvent returns null for incomplete payload`() {
        // content_liked requires both content_type and content_id
        val storedEvent = StoredEvent(
            seq = 100,
            type = "content_liked",
            payload = SyncEventPayload(
                contentType = LikedContentType.Album,
                // missing content_id
            ),
            serverTimestamp = 1701700000,
        )

        val event = storedEvent.toSyncEvent()

        assertThat(event).isNull()
    }

    // endregion

    // region StoredEvent serialization roundtrip

    @Test
    fun `StoredEvent serialization roundtrip works`() {
        val original = StoredEvent(
            seq = 99,
            type = "content_liked",
            payload = SyncEventPayload(
                contentType = LikedContentType.Track,
                contentId = "track_999",
            ),
            serverTimestamp = 1701700099,
        )

        val serialized = json.encodeToString(StoredEvent.serializer(), original)
        val deserialized = json.decodeFromString<StoredEvent>(serialized)

        assertThat(deserialized.seq).isEqualTo(original.seq)
        assertThat(deserialized.type).isEqualTo(original.type)
        assertThat(deserialized.serverTimestamp).isEqualTo(original.serverTimestamp)
        assertThat(deserialized.payload.contentType).isEqualTo(original.payload.contentType)
        assertThat(deserialized.payload.contentId).isEqualTo(original.payload.contentId)
    }

    // endregion

    // region SyncEventsResponse deserialization

    @Test
    fun `deserialize SyncEventsResponse with events`() {
        val jsonString = """
            {
                "events": [
                    {
                        "seq": 1,
                        "type": "content_liked",
                        "payload": {"content_type": "album", "content_id": "album_1"},
                        "server_timestamp": 1701700001
                    },
                    {
                        "seq": 2,
                        "type": "content_unliked",
                        "payload": {"content_type": "track", "content_id": "track_1"},
                        "server_timestamp": 1701700002
                    }
                ],
                "current_seq": 2
            }
        """.trimIndent()

        val response = json.decodeFromString<com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse>(jsonString)

        assertThat(response.currentSeq).isEqualTo(2L)
        assertThat(response.events).hasSize(2)
        assertThat(response.events[0].seq).isEqualTo(1L)
        assertThat(response.events[1].seq).isEqualTo(2L)
    }

    @Test
    fun `deserialize SyncEventsResponse with empty events`() {
        val jsonString = """
            {
                "events": [],
                "current_seq": 10
            }
        """.trimIndent()

        val response = json.decodeFromString<com.lelloman.pezzottify.android.domain.remoteapi.response.SyncEventsResponse>(jsonString)

        assertThat(response.currentSeq).isEqualTo(10L)
        assertThat(response.events).isEmpty()
    }

    // endregion
}
