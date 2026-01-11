package com.lelloman.pezzottify.android.assistant.tools

import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType
import com.lelloman.pezzottify.android.domain.statics.DiscographyProvider
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWhatsNew
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.simpleaiassistant.tool.Tool
import com.lelloman.simpleaiassistant.tool.ToolResult
import com.lelloman.simpleaiassistant.tool.ToolSpec
import kotlinx.coroutines.flow.firstOrNull
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Tool to search the music catalog for tracks, albums, and artists.
 * By default, excludes unavailable content (most users want to play music).
 * Set include_unavailable=true to explore/discover unavailable content.
 */
class SearchCatalogTool(
    private val performSearch: PerformSearch,
    private val staticsStore: StaticsStore
) : Tool {
    override val spec = ToolSpec(
        name = "search_catalog",
        description = "Search the music catalog for tracks, albums, and artists. Only returns playable content by default - unavailable content is excluded. Use include_unavailable=true only for discovery/exploration purposes.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "query" to mapOf(
                    "type" to "string",
                    "description" to "The search query (artist name, album name, track name, etc.)"
                ),
                "filter" to mapOf(
                    "type" to "string",
                    "enum" to listOf("all", "tracks", "albums", "artists"),
                    "description" to "Optional filter to limit results to a specific type"
                ),
                "limit" to mapOf(
                    "type" to "integer",
                    "description" to "Maximum number of results to return (default: 10)"
                ),
                "include_unavailable" to mapOf(
                    "type" to "boolean",
                    "description" to "If true, include unavailable content in results (for exploration/discovery). Default: false"
                )
            ),
            "required" to listOf("query")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val query = input["query"] as? String
            ?: return ToolResult(success = false, error = "Missing query parameter")

        val filterInput = input["filter"] as? String ?: "all"
        val limit = (input["limit"] as? Number)?.toInt() ?: 10
        val includeUnavailable = input["include_unavailable"] as? Boolean ?: false

        val filters: List<RemoteApiClient.SearchFilter>? = when (filterInput.lowercase()) {
            "track", "tracks" -> listOf(RemoteApiClient.SearchFilter.Track)
            "album", "albums" -> listOf(RemoteApiClient.SearchFilter.Album)
            "artist", "artists" -> listOf(RemoteApiClient.SearchFilter.Artist)
            else -> null // "all" - no filter
        }

        return performSearch(query, filters, excludeUnavailable = !includeUnavailable).fold(
            onSuccess = { results ->
                if (results.isEmpty()) {
                    val hint = if (!includeUnavailable) " Try with include_unavailable=true to search all content." else ""
                    ToolResult(success = true, data = "No results found for \"$query\".$hint")
                } else {
                    val limitedResults = results.take(limit)
                    val formatted = buildString {
                        appendLine("Found ${results.size} results for \"$query\":")
                        limitedResults.forEach { (id, type) ->
                            val displayInfo = getDisplayInfo(id, type)
                            appendLine("- $displayInfo")
                        }
                        if (results.size > limit) {
                            appendLine("(showing first $limit of ${results.size} results)")
                        }
                    }
                    ToolResult(success = true, data = formatted.trimEnd())
                }
            },
            onFailure = { error ->
                ToolResult(success = false, error = "Search failed: ${error.message ?: "Unknown error"}")
            }
        )
    }

    private suspend fun getDisplayInfo(id: String, type: SearchedItemType): String {
        return when (type) {
            SearchedItemType.Track -> {
                val track = staticsStore.getTrack(id).firstOrNull()
                if (track != null) {
                    val artistNames = getArtistNames(track.artistsIds)
                    "[Track] \"${track.name}\" by $artistNames (ID: $id)"
                } else {
                    "[Track] ID: $id"
                }
            }
            SearchedItemType.Album -> {
                val album = staticsStore.getAlbum(id).firstOrNull()
                if (album != null) {
                    val artistNames = getArtistNames(album.artistsIds)
                    "[Album] \"${album.name}\" by $artistNames (ID: $id)"
                } else {
                    "[Album] ID: $id"
                }
            }
            SearchedItemType.Artist -> {
                val artist = staticsStore.getArtist(id).firstOrNull()
                if (artist != null) {
                    "[Artist] \"${artist.name}\" (ID: $id)"
                } else {
                    "[Artist] ID: $id"
                }
            }
        }
    }

    private suspend fun getArtistNames(artistIds: List<String>): String {
        val names = artistIds.mapNotNull { staticsStore.getArtist(it).firstOrNull()?.name }
        return if (names.isNotEmpty()) names.joinToString(", ") else "Unknown Artist"
    }
}

/**
 * Tool to get an artist's discography (albums) by artist ID.
 * Use this after finding an artist via search_catalog to get their albums.
 */
class GetArtistDiscographyTool(
    private val discographyProvider: DiscographyProvider,
    private val staticsStore: StaticsStore
) : Tool {
    override val spec = ToolSpec(
        name = "get_artist_discography",
        description = "Get an artist's albums by artist ID (from search_catalog). Returns album IDs that can be used with play_album.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "artist_id" to mapOf(
                    "type" to "string",
                    "description" to "The artist ID to get albums for"
                )
            ),
            "required" to listOf("artist_id")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val artistId = input["artist_id"] as? String
            ?: return ToolResult(success = false, error = "Missing artist_id parameter")

        val artist = staticsStore.getArtist(artistId).firstOrNull()
        val albumIds = discographyProvider.getAlbumIdsForArtist(artistId)

        if (albumIds.isEmpty()) {
            return ToolResult(
                success = false,
                error = "No albums found for artist ID: $artistId"
            )
        }

        // Fetch all albums and sort by date (newest first)
        val albums = albumIds.mapNotNull { albumId ->
            staticsStore.getAlbum(albumId).firstOrNull()?.let { albumId to it }
        }.sortedByDescending { it.second.date }

        val formatted = buildString {
            val artistName = artist?.name ?: artistId
            appendLine("Discography for \"$artistName\" (${albums.size} albums):")
            appendLine()
            for ((albumId, album) in albums) {
                val year = if (album.date > 0) " (${album.date})" else ""
                appendLine("- \"${album.name}\"$year (ID: $albumId)")
            }
            // Show any albums we couldn't fetch
            val missingIds = albumIds.filter { id -> albums.none { it.first == id } }
            for (albumId in missingIds) {
                appendLine("- Album ID: $albumId")
            }
        }

        return ToolResult(success = true, data = formatted.trimEnd())
    }
}

/**
 * Tool to get the latest releases / what's new in the catalog.
 */
class WhatsNewTool(
    private val getWhatsNew: GetWhatsNew
) : Tool {
    override val spec = ToolSpec(
        name = "whats_new",
        description = "Get the latest releases and recent additions to the music catalog. Use this when the user asks about new music, latest releases, or what's new.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "limit" to mapOf(
                    "type" to "integer",
                    "description" to "Maximum number of recent batches to return (default: 5)"
                )
            )
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val limit = (input["limit"] as? Number)?.toInt() ?: 5

        return getWhatsNew(limit).fold(
            onSuccess = { response ->
                if (response.batches.isEmpty()) {
                    ToolResult(success = true, data = "No recent updates found in the catalog.")
                } else {
                    val dateFormat = SimpleDateFormat("MMM d, yyyy", Locale.getDefault())
                    val formatted = buildString {
                        appendLine("Recent catalog updates:")
                        response.batches.forEach { batch ->
                            val date = dateFormat.format(Date(batch.closedAt * 1000))
                            appendLine()
                            appendLine("📦 ${batch.name} ($date)")
                            if (batch.description != null) {
                                appendLine("   ${batch.description}")
                            }

                            // Show added albums with names
                            val addedAlbums = batch.summary.albums.added
                            if (addedAlbums.isNotEmpty()) {
                                appendLine("   New albums:")
                                addedAlbums.forEach { album ->
                                    appendLine("   - \"${album.name}\" (ID: ${album.id})")
                                }
                            }

                            // Show added artists with names
                            val addedArtists = batch.summary.artists.added
                            if (addedArtists.isNotEmpty()) {
                                appendLine("   New artists:")
                                addedArtists.forEach { artist ->
                                    appendLine("   - \"${artist.name}\" (ID: ${artist.id})")
                                }
                            }

                            // Show track counts
                            val tracksAdded = batch.summary.tracks.addedCount
                            if (tracksAdded > 0) {
                                appendLine("   $tracksAdded new tracks added")
                            }
                        }
                    }
                    ToolResult(success = true, data = formatted.trimEnd())
                }
            },
            onFailure = { error ->
                ToolResult(success = false, error = "Failed to get latest releases: ${error.message ?: "Unknown error"}")
            }
        )
    }
}
