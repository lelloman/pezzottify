package com.lelloman.pezzottify.android.remoteapi.internal.requests

import com.google.common.truth.Truth.assertThat
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import org.junit.Test

@OptIn(ExperimentalSerializationApi::class)
class BatchContentResponseTest {

    // Match the JSON configuration used in RemoteApiClientImpl
    private val json = Json {
        ignoreUnknownKeys = true
        namingStrategy = JsonNamingStrategy.SnakeCase
        encodeDefaults = true
    }

    // Full configuration as used in RemoteApiClientImpl
    private val jsonWithDiscriminator = Json {
        ignoreUnknownKeys = true
        namingStrategy = JsonNamingStrategy.SnakeCase
        classDiscriminator = "section"
        encodeDefaults = true
    }

    @Test
    fun `deserializes successful artist result`() {
        val jsonString = """
            {
                "artists": {
                    "artist-1": {
                        "ok": {
                            "artist": {
                                "id": "artist-1",
                                "name": "Test Artist",
                                "genres": ["rock", "pop"]
                            },
                            "related_artists": []
                        }
                    }
                },
                "albums": {},
                "tracks": {}
            }
        """.trimIndent()

        val response = json.decodeFromString<BatchContentResponse>(jsonString)

        assertThat(response.artists).hasSize(1)
        val artistResult = response.artists["artist-1"]
        assertThat(artistResult).isInstanceOf(BatchArtistResult.Ok::class.java)
        val ok = artistResult as BatchArtistResult.Ok
        assertThat(ok.ok.artist.id).isEqualTo("artist-1")
        assertThat(ok.ok.artist.name).isEqualTo("Test Artist")
    }

    @Test
    fun `deserializes error artist result`() {
        val jsonString = """
            {
                "artists": {
                    "nonexistent": {
                        "error": "not_found"
                    }
                },
                "albums": {},
                "tracks": {}
            }
        """.trimIndent()

        val response = json.decodeFromString<BatchContentResponse>(jsonString)

        assertThat(response.artists).hasSize(1)
        val artistResult = response.artists["nonexistent"]
        assertThat(artistResult).isInstanceOf(BatchArtistResult.Error::class.java)
        val error = artistResult as BatchArtistResult.Error
        assertThat(error.error).isEqualTo("not_found")
    }

    @Test
    fun `deserializes successful album result`() {
        // Images are fetched by album ID via /v1/content/image/{id} endpoint
        val jsonString = """
            {
                "artists": {},
                "albums": {
                    "album-1": {
                        "ok": {
                            "album": {
                                "id": "album-1",
                                "name": "Test Album",
                                "album_type": "album",
                                "release_date": "2023-01-15"
                            },
                            "artists": [
                                {"id": "artist-1", "name": "Test Artist", "genres": []}
                            ],
                            "discs": [
                                {"number": 1, "tracks": []}
                            ]
                        }
                    }
                },
                "tracks": {}
            }
        """.trimIndent()

        val response = json.decodeFromString<BatchContentResponse>(jsonString)

        assertThat(response.albums).hasSize(1)
        val albumResult = response.albums["album-1"]
        assertThat(albumResult).isInstanceOf(BatchAlbumResult.Ok::class.java)
        val ok = albumResult as BatchAlbumResult.Ok
        assertThat(ok.ok.album.id).isEqualTo("album-1")
        assertThat(ok.ok.album.name).isEqualTo("Test Album")
    }

    @Test
    fun `deserializes successful track result`() {
        val jsonString = """
            {
                "artists": {},
                "albums": {},
                "tracks": {
                    "track-1": {
                        "ok": {
                            "track": {
                                "id": "track-1",
                                "name": "Test Track",
                                "album_id": "album-1",
                                "disc_number": 1,
                                "track_number": 1,
                                "duration_ms": 240000
                            },
                            "album": {
                                "id": "album-1",
                                "name": "Test Album",
                                "album_type": "album",
                                "release_date": "2023"
                            },
                            "artists": [
                                {"artist": {"id": "artist-1", "name": "Test Artist", "genres": []}, "role": "MainArtist"}
                            ]
                        }
                    }
                }
            }
        """.trimIndent()

        val response = json.decodeFromString<BatchContentResponse>(jsonString)

        assertThat(response.tracks).hasSize(1)
        val trackResult = response.tracks["track-1"]
        assertThat(trackResult).isInstanceOf(BatchTrackResult.Ok::class.java)
        val ok = trackResult as BatchTrackResult.Ok
        assertThat(ok.ok.track.id).isEqualTo("track-1")
        assertThat(ok.ok.track.name).isEqualTo("Test Track")
        assertThat(ok.ok.track.durationMs).isEqualTo(240000)
    }

    @Test
    fun `deserializes mixed success and error results`() {
        val jsonString = """
            {
                "artists": {
                    "artist-1": {
                        "ok": {
                            "artist": {"id": "artist-1", "name": "Found Artist", "genres": []},
                            "related_artists": []
                        }
                    },
                    "artist-2": {
                        "error": "not_found"
                    }
                },
                "albums": {},
                "tracks": {}
            }
        """.trimIndent()

        val response = json.decodeFromString<BatchContentResponse>(jsonString)

        assertThat(response.artists).hasSize(2)

        val found = response.artists["artist-1"]
        assertThat(found).isInstanceOf(BatchArtistResult.Ok::class.java)

        val notFound = response.artists["artist-2"]
        assertThat(notFound).isInstanceOf(BatchArtistResult.Error::class.java)
        assertThat((notFound as BatchArtistResult.Error).error).isEqualTo("not_found")
    }

    @Test
    fun `deserializes empty response`() {
        val jsonString = """
            {
                "artists": {},
                "albums": {},
                "tracks": {}
            }
        """.trimIndent()

        val response = json.decodeFromString<BatchContentResponse>(jsonString)

        assertThat(response.artists).isEmpty()
        assertThat(response.albums).isEmpty()
        assertThat(response.tracks).isEmpty()
    }

    @Test
    fun `serializes batch request correctly`() {
        val request = BatchContentRequest(
            artists = listOf(BatchItemRequest("artist-1", resolved = true)),
            albums = listOf(BatchItemRequest("album-1", resolved = false)),
            tracks = emptyList()
        )

        val jsonString = json.encodeToString(BatchContentRequest.serializer(), request)

        assertThat(jsonString).contains("\"id\":\"artist-1\"")
        assertThat(jsonString).contains("\"id\":\"album-1\"")
        // Both resolved values should be serialized with encodeDefaults=true
        assertThat(jsonString).contains("\"resolved\":false")
        assertThat(jsonString).contains("\"resolved\":true")
    }

    @Test
    fun `deserializes with classDiscriminator config (matching RemoteApiClientImpl)`() {
        val jsonString = """
            {
                "artists": {
                    "artist-1": {
                        "ok": {
                            "artist": {
                                "id": "artist-1",
                                "name": "Test Artist",
                                "genres": ["rock", "pop"]
                            },
                            "related_artists": []
                        }
                    }
                },
                "albums": {},
                "tracks": {}
            }
        """.trimIndent()

        val response = jsonWithDiscriminator.decodeFromString<BatchContentResponse>(jsonString)

        assertThat(response.artists).hasSize(1)
        val artistResult = response.artists["artist-1"]
        assertThat(artistResult).isInstanceOf(BatchArtistResult.Ok::class.java)
        val ok = artistResult as BatchArtistResult.Ok
        assertThat(ok.ok.artist.id).isEqualTo("artist-1")
        assertThat(ok.ok.artist.name).isEqualTo("Test Artist")
    }
}
