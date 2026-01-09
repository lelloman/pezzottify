package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.google.common.truth.Truth.assertThat
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import org.junit.Test

@OptIn(ExperimentalSerializationApi::class)
class StreamingSearchResponseTest {

    // Match the server's JSON format: uses "section" as discriminator for SearchSection
    private val json = Json {
        ignoreUnknownKeys = true
        namingStrategy = JsonNamingStrategy.SnakeCase
        classDiscriminator = "section"
    }

    @Test
    fun `deserializes primary_artist section`() {
        // Server uses internally tagged format with "type" discriminator for ResolvedSearchResult
        val jsonString = """
            {
                "section": "primary_artist",
                "item": {
                    "type": "Artist",
                    "id": "artist-123",
                    "name": "Test Artist",
                    "image_id": "img-456"
                },
                "confidence": 0.95
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.PrimaryArtist::class.java)
        val primaryArtist = result as SearchSection.PrimaryArtist
        assertThat(primaryArtist.confidence).isEqualTo(0.95)
        assertThat(primaryArtist.item).isInstanceOf(ResolvedSearchResult.Artist::class.java)
        val artist = primaryArtist.item as ResolvedSearchResult.Artist
        assertThat(artist.id).isEqualTo("artist-123")
        assertThat(artist.name).isEqualTo("Test Artist")
        assertThat(artist.imageId).isEqualTo("img-456")
    }

    @Test
    fun `deserializes primary_album section`() {
        val jsonString = """
            {
                "section": "primary_album",
                "item": {
                    "type": "Album",
                    "id": "album-123",
                    "name": "Test Album",
                    "artists_ids_names": [["artist-1", "Artist Name"]],
                    "image_id": "img-789",
                    "year": 2023,
                    "availability": "Available"
                },
                "confidence": 0.88
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.PrimaryAlbum::class.java)
        val primaryAlbum = result as SearchSection.PrimaryAlbum
        assertThat(primaryAlbum.confidence).isEqualTo(0.88)
        val album = primaryAlbum.item as ResolvedSearchResult.Album
        assertThat(album.id).isEqualTo("album-123")
        assertThat(album.name).isEqualTo("Test Album")
        assertThat(album.year).isEqualTo(2023)
    }

    @Test
    fun `deserializes primary_track section`() {
        val jsonString = """
            {
                "section": "primary_track",
                "item": {
                    "type": "Track",
                    "id": "track-123",
                    "name": "Test Track",
                    "duration": 180,
                    "artists_ids_names": [["artist-1", "Artist Name"]],
                    "album_id": "album-1",
                    "availability": "Available"
                },
                "confidence": 0.92
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.PrimaryTrack::class.java)
        val primaryTrack = result as SearchSection.PrimaryTrack
        assertThat(primaryTrack.confidence).isEqualTo(0.92)
        val track = primaryTrack.item as ResolvedSearchResult.Track
        assertThat(track.id).isEqualTo("track-123")
        assertThat(track.duration).isEqualTo(180)
        assertThat(track.albumId).isEqualTo("album-1")
    }

    @Test
    fun `deserializes popular_by section`() {
        val jsonString = """
            {
                "section": "popular_by",
                "target_id": "artist-123",
                "target_type": "artist",
                "items": [
                    {
                        "id": "track-1",
                        "name": "Popular Track 1",
                        "duration_ms": 180000,
                        "track_number": 1,
                        "album_id": "album-1",
                        "album_name": "Album One",
                        "artist_names": ["Artist Name"],
                        "image_id": "img-1"
                    }
                ]
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.PopularBy::class.java)
        val popularBy = result as SearchSection.PopularBy
        assertThat(popularBy.targetId).isEqualTo("artist-123")
        assertThat(popularBy.targetType).isEqualTo(MatchType.Artist)
        assertThat(popularBy.items).hasSize(1)
        assertThat(popularBy.items[0].name).isEqualTo("Popular Track 1")
        assertThat(popularBy.items[0].durationMs).isEqualTo(180000)
    }

    @Test
    fun `deserializes albums_by section`() {
        val jsonString = """
            {
                "section": "albums_by",
                "target_id": "artist-123",
                "items": [
                    {
                        "id": "album-1",
                        "name": "First Album",
                        "release_year": 2020,
                        "track_count": 12,
                        "image_id": "img-1",
                        "artist_names": ["Artist Name"],
                        "availability": "Available"
                    }
                ]
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.AlbumsBy::class.java)
        val albumsBy = result as SearchSection.AlbumsBy
        assertThat(albumsBy.targetId).isEqualTo("artist-123")
        assertThat(albumsBy.items).hasSize(1)
        assertThat(albumsBy.items[0].releaseYear).isEqualTo(2020)
        assertThat(albumsBy.items[0].trackCount).isEqualTo(12)
    }

    @Test
    fun `deserializes tracks_from section`() {
        val jsonString = """
            {
                "section": "tracks_from",
                "target_id": "album-123",
                "items": [
                    {
                        "id": "track-1",
                        "name": "Track One",
                        "duration_ms": 200000,
                        "track_number": 1,
                        "album_id": "album-123",
                        "album_name": "Test Album",
                        "artist_names": ["Artist"]
                    }
                ]
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.TracksFrom::class.java)
        val tracksFrom = result as SearchSection.TracksFrom
        assertThat(tracksFrom.targetId).isEqualTo("album-123")
        assertThat(tracksFrom.items).hasSize(1)
    }

    @Test
    fun `deserializes related_artists section`() {
        val jsonString = """
            {
                "section": "related_artists",
                "target_id": "artist-123",
                "items": [
                    {
                        "id": "artist-2",
                        "name": "Related Artist",
                        "image_id": "img-2"
                    }
                ]
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.RelatedArtists::class.java)
        val relatedArtists = result as SearchSection.RelatedArtists
        assertThat(relatedArtists.targetId).isEqualTo("artist-123")
        assertThat(relatedArtists.items).hasSize(1)
        assertThat(relatedArtists.items[0].name).isEqualTo("Related Artist")
    }

    @Test
    fun `deserializes more_results section`() {
        val jsonString = """
            {
                "section": "more_results",
                "items": [
                    {
                        "type": "Track",
                        "id": "track-2",
                        "name": "Another Track",
                        "duration": 240,
                        "artists_ids_names": [["a1", "Artist"]],
                        "album_id": "album-2",
                        "availability": "Available"
                    },
                    {
                        "type": "Album",
                        "id": "album-3",
                        "name": "Another Album",
                        "artists_ids_names": [["a1", "Artist"]],
                        "availability": "Available"
                    }
                ]
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.MoreResults::class.java)
        val moreResults = result as SearchSection.MoreResults
        assertThat(moreResults.items).hasSize(2)
        assertThat(moreResults.items[0]).isInstanceOf(ResolvedSearchResult.Track::class.java)
        assertThat(moreResults.items[1]).isInstanceOf(ResolvedSearchResult.Album::class.java)
    }

    @Test
    fun `deserializes results section`() {
        val jsonString = """
            {
                "section": "results",
                "items": [
                    {
                        "type": "Artist",
                        "id": "artist-1",
                        "name": "Some Artist"
                    }
                ]
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.Results::class.java)
        val results = result as SearchSection.Results
        assertThat(results.items).hasSize(1)
        assertThat(results.items[0]).isInstanceOf(ResolvedSearchResult.Artist::class.java)
    }

    @Test
    fun `deserializes done section`() {
        val jsonString = """
            {
                "section": "done",
                "total_time_ms": 150
            }
        """.trimIndent()

        val result = json.decodeFromString<SearchSection>(jsonString)

        assertThat(result).isInstanceOf(SearchSection.Done::class.java)
        val done = result as SearchSection.Done
        assertThat(done.totalTimeMs).isEqualTo(150)
    }

    @Test
    fun `deserializes MatchType values correctly`() {
        assertThat(json.decodeFromString<MatchType>("\"artist\"")).isEqualTo(MatchType.Artist)
        assertThat(json.decodeFromString<MatchType>("\"album\"")).isEqualTo(MatchType.Album)
        assertThat(json.decodeFromString<MatchType>("\"track\"")).isEqualTo(MatchType.Track)
    }

    @Test
    fun `TrackSummary handles optional fields`() {
        val jsonString = """
            {
                "id": "track-1",
                "name": "Track Name",
                "duration_ms": 180000,
                "album_id": "album-1",
                "album_name": "Album Name",
                "artist_names": ["Artist"]
            }
        """.trimIndent()

        val result = json.decodeFromString<TrackSummary>(jsonString)

        assertThat(result.trackNumber).isNull()
        assertThat(result.imageId).isNull()
    }

    @Test
    fun `AlbumSummary handles optional fields`() {
        val jsonString = """
            {
                "id": "album-1",
                "name": "Album Name",
                "track_count": 10,
                "artist_names": ["Artist"],
                "availability": "Available"
            }
        """.trimIndent()

        val result = json.decodeFromString<AlbumSummary>(jsonString)

        assertThat(result.releaseYear).isNull()
        assertThat(result.imageId).isNull()
    }
}
