package com.lelloman.pezzottify.android.assistant.tools

import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.simpleaiassistant.tool.Tool
import com.lelloman.simpleaiassistant.tool.ToolResult
import com.lelloman.simpleaiassistant.tool.ToolSpec
import kotlinx.coroutines.flow.firstOrNull
import java.util.UUID

/**
 * Tool to list all user playlists.
 */
class ListPlaylistsTool(
    private val userPlaylistStore: UserPlaylistStore
) : Tool {
    override val spec = ToolSpec(
        name = "list_playlists",
        description = "Get a list of all user playlists with their IDs, names, and track counts.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf<String, Any>()
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val playlists = userPlaylistStore.getPlaylists().firstOrNull() ?: emptyList()

        if (playlists.isEmpty()) {
            return ToolResult(success = true, data = "No playlists found. Use create_playlist to create one.")
        }

        val formatted = buildString {
            appendLine("Your playlists (${playlists.size}):")
            playlists.forEach { playlist ->
                val syncInfo = when (playlist.syncStatus) {
                    PlaylistSyncStatus.Synced -> ""
                    PlaylistSyncStatus.PendingCreate -> " [pending create]"
                    PlaylistSyncStatus.PendingUpdate -> " [pending update]"
                    PlaylistSyncStatus.PendingDelete -> " [pending delete]"
                    PlaylistSyncStatus.Syncing -> " [syncing...]"
                    PlaylistSyncStatus.SyncError -> " [sync error]"
                }
                appendLine("- \"${playlist.name}\" (${playlist.trackIds.size} tracks) ID: ${playlist.id}$syncInfo")
            }
        }

        return ToolResult(success = true, data = formatted.trimEnd())
    }
}

/**
 * Tool to view a playlist's tracks.
 */
class ViewPlaylistTool(
    private val userPlaylistStore: UserPlaylistStore,
    private val staticsStore: StaticsStore
) : Tool {
    override val spec = ToolSpec(
        name = "view_playlist",
        description = "View the tracks in a specific playlist by its ID.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "playlist_id" to mapOf(
                    "type" to "string",
                    "description" to "The playlist ID to view"
                )
            ),
            "required" to listOf("playlist_id")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val playlistId = input["playlist_id"] as? String
            ?: return ToolResult(success = false, error = "Missing playlist_id parameter")

        val playlist = userPlaylistStore.getPlaylist(playlistId).firstOrNull()
            ?: return ToolResult(success = false, error = "Playlist not found: $playlistId")

        if (playlist.trackIds.isEmpty()) {
            return ToolResult(
                success = true,
                data = "Playlist \"${playlist.name}\" is empty. Use add_tracks_to_playlist to add tracks."
            )
        }

        val formatted = buildString {
            appendLine("Playlist \"${playlist.name}\" (${playlist.trackIds.size} tracks):")
            playlist.trackIds.forEachIndexed { index, trackId ->
                val track = staticsStore.getTrack(trackId).firstOrNull()
                if (track != null) {
                    val artistNames = getArtistNames(track.artistsIds)
                    appendLine("${index + 1}. \"${track.name}\" by $artistNames (ID: $trackId)")
                } else {
                    appendLine("${index + 1}. Track ID: $trackId")
                }
            }
        }

        return ToolResult(success = true, data = formatted.trimEnd())
    }

    private suspend fun getArtistNames(artistIds: List<String>): String {
        val names = artistIds.mapNotNull { staticsStore.getArtist(it).firstOrNull()?.name }
        return if (names.isNotEmpty()) names.joinToString(", ") else "Unknown Artist"
    }
}

/**
 * Tool to create a new playlist.
 */
class CreatePlaylistTool(
    private val userPlaylistStore: UserPlaylistStore
) : Tool {
    override val spec = ToolSpec(
        name = "create_playlist",
        description = "Create a new playlist with a given name. Optionally add initial tracks.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "name" to mapOf(
                    "type" to "string",
                    "description" to "The name for the new playlist"
                ),
                "track_ids" to mapOf(
                    "type" to "array",
                    "items" to mapOf("type" to "string"),
                    "description" to "Optional: Initial track IDs to add to the playlist"
                )
            ),
            "required" to listOf("name")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val name = input["name"] as? String
            ?: return ToolResult(success = false, error = "Missing name parameter")

        @Suppress("UNCHECKED_CAST")
        val trackIds = (input["track_ids"] as? List<*>)?.mapNotNull { it as? String } ?: emptyList()

        val playlistId = UUID.randomUUID().toString()

        userPlaylistStore.createOrUpdatePlaylist(
            id = playlistId,
            name = name,
            trackIds = trackIds,
            syncStatus = PlaylistSyncStatus.PendingCreate
        )

        val trackInfo = if (trackIds.isNotEmpty()) " with ${trackIds.size} tracks" else ""
        return ToolResult(
            success = true,
            data = "Created playlist \"$name\"$trackInfo (ID: $playlistId)"
        )
    }
}

/**
 * Tool to rename a playlist.
 */
class RenamePlaylistTool(
    private val userPlaylistStore: UserPlaylistStore
) : Tool {
    override val spec = ToolSpec(
        name = "rename_playlist",
        description = "Rename an existing playlist.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "playlist_id" to mapOf(
                    "type" to "string",
                    "description" to "The playlist ID to rename"
                ),
                "new_name" to mapOf(
                    "type" to "string",
                    "description" to "The new name for the playlist"
                )
            ),
            "required" to listOf("playlist_id", "new_name")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val playlistId = input["playlist_id"] as? String
            ?: return ToolResult(success = false, error = "Missing playlist_id parameter")
        val newName = input["new_name"] as? String
            ?: return ToolResult(success = false, error = "Missing new_name parameter")

        val playlist = userPlaylistStore.getPlaylist(playlistId).firstOrNull()
            ?: return ToolResult(success = false, error = "Playlist not found: $playlistId")

        val oldName = playlist.name
        userPlaylistStore.updatePlaylistName(playlistId, newName)

        return ToolResult(
            success = true,
            data = "Renamed playlist from \"$oldName\" to \"$newName\""
        )
    }
}

/**
 * Tool to delete a playlist.
 */
class DeletePlaylistTool(
    private val userPlaylistStore: UserPlaylistStore
) : Tool {
    override val spec = ToolSpec(
        name = "delete_playlist",
        description = "Delete a playlist by its ID. This action cannot be undone.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "playlist_id" to mapOf(
                    "type" to "string",
                    "description" to "The playlist ID to delete"
                )
            ),
            "required" to listOf("playlist_id")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val playlistId = input["playlist_id"] as? String
            ?: return ToolResult(success = false, error = "Missing playlist_id parameter")

        val playlist = userPlaylistStore.getPlaylist(playlistId).firstOrNull()
            ?: return ToolResult(success = false, error = "Playlist not found: $playlistId")

        val name = playlist.name
        userPlaylistStore.markPlaylistForDeletion(playlistId)

        return ToolResult(success = true, data = "Deleted playlist \"$name\"")
    }
}

/**
 * Tool to add tracks to a playlist.
 */
class AddTracksToPlaylistTool(
    private val userPlaylistStore: UserPlaylistStore,
    private val staticsStore: StaticsStore
) : Tool {
    override val spec = ToolSpec(
        name = "add_tracks_to_playlist",
        description = "Add one or more tracks to a playlist. Get track IDs from search_catalog.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "playlist_id" to mapOf(
                    "type" to "string",
                    "description" to "The playlist ID to add tracks to"
                ),
                "track_ids" to mapOf(
                    "type" to "array",
                    "items" to mapOf("type" to "string"),
                    "description" to "The track IDs to add to the playlist"
                )
            ),
            "required" to listOf("playlist_id", "track_ids")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val playlistId = input["playlist_id"] as? String
            ?: return ToolResult(success = false, error = "Missing playlist_id parameter")

        @Suppress("UNCHECKED_CAST")
        val trackIds = (input["track_ids"] as? List<*>)?.mapNotNull { it as? String }
            ?: return ToolResult(success = false, error = "Missing or invalid track_ids parameter")

        if (trackIds.isEmpty()) {
            return ToolResult(success = false, error = "track_ids cannot be empty")
        }

        val playlist = userPlaylistStore.getPlaylist(playlistId).firstOrNull()
            ?: return ToolResult(success = false, error = "Playlist not found: $playlistId")

        userPlaylistStore.addTracksToPlaylist(playlistId, trackIds)

        // Get track names for response
        val trackNames = trackIds.take(3).mapNotNull { trackId ->
            staticsStore.getTrack(trackId).firstOrNull()?.name
        }
        val trackInfo = if (trackNames.isNotEmpty()) {
            val names = trackNames.joinToString(", ") { "\"$it\"" }
            if (trackIds.size > 3) "$names and ${trackIds.size - 3} more" else names
        } else {
            "${trackIds.size} tracks"
        }

        return ToolResult(
            success = true,
            data = "Added $trackInfo to playlist \"${playlist.name}\""
        )
    }
}

/**
 * Tool to remove tracks from a playlist.
 */
class RemoveTracksFromPlaylistTool(
    private val userPlaylistStore: UserPlaylistStore,
    private val staticsStore: StaticsStore
) : Tool {
    override val spec = ToolSpec(
        name = "remove_tracks_from_playlist",
        description = "Remove one or more tracks from a playlist.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "playlist_id" to mapOf(
                    "type" to "string",
                    "description" to "The playlist ID to remove tracks from"
                ),
                "track_ids" to mapOf(
                    "type" to "array",
                    "items" to mapOf("type" to "string"),
                    "description" to "The track IDs to remove from the playlist"
                )
            ),
            "required" to listOf("playlist_id", "track_ids")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val playlistId = input["playlist_id"] as? String
            ?: return ToolResult(success = false, error = "Missing playlist_id parameter")

        @Suppress("UNCHECKED_CAST")
        val trackIds = (input["track_ids"] as? List<*>)?.mapNotNull { it as? String }
            ?: return ToolResult(success = false, error = "Missing or invalid track_ids parameter")

        if (trackIds.isEmpty()) {
            return ToolResult(success = false, error = "track_ids cannot be empty")
        }

        val playlist = userPlaylistStore.getPlaylist(playlistId).firstOrNull()
            ?: return ToolResult(success = false, error = "Playlist not found: $playlistId")

        // Remove each track
        for (trackId in trackIds) {
            userPlaylistStore.removeTrackFromPlaylist(playlistId, trackId)
        }

        return ToolResult(
            success = true,
            data = "Removed ${trackIds.size} track(s) from playlist \"${playlist.name}\""
        )
    }
}

/**
 * Tool to play a playlist.
 */
class PlayPlaylistTool(
    private val player: PezzottifyPlayer,
    private val userPlaylistStore: UserPlaylistStore
) : Tool {
    override val spec = ToolSpec(
        name = "play_playlist",
        description = "Play a playlist by its ID, replacing the current queue.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "playlist_id" to mapOf(
                    "type" to "string",
                    "description" to "The playlist ID to play"
                ),
                "start_track_id" to mapOf(
                    "type" to "string",
                    "description" to "Optional: Start playing from a specific track in the playlist"
                )
            ),
            "required" to listOf("playlist_id")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val playlistId = input["playlist_id"] as? String
            ?: return ToolResult(success = false, error = "Missing playlist_id parameter")
        val startTrackId = input["start_track_id"] as? String

        val playlist = userPlaylistStore.getPlaylist(playlistId).firstOrNull()
            ?: return ToolResult(success = false, error = "Playlist not found: $playlistId")

        if (playlist.trackIds.isEmpty()) {
            return ToolResult(success = false, error = "Playlist \"${playlist.name}\" is empty")
        }

        player.loadUserPlaylist(playlistId, startTrackId)

        return ToolResult(
            success = true,
            data = "Started playing playlist \"${playlist.name}\" (${playlist.trackIds.size} tracks)"
        )
    }
}

/**
 * Tool to add a playlist to the current queue without replacing it.
 */
class AddPlaylistToQueueTool(
    private val player: PezzottifyPlayer,
    private val userPlaylistStore: UserPlaylistStore
) : Tool {
    override val spec = ToolSpec(
        name = "add_playlist_to_queue",
        description = "Add all tracks from a playlist to the end of the current playback queue.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "playlist_id" to mapOf(
                    "type" to "string",
                    "description" to "The playlist ID to add to the queue"
                )
            ),
            "required" to listOf("playlist_id")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val playlistId = input["playlist_id"] as? String
            ?: return ToolResult(success = false, error = "Missing playlist_id parameter")

        val playlist = userPlaylistStore.getPlaylist(playlistId).firstOrNull()
            ?: return ToolResult(success = false, error = "Playlist not found: $playlistId")

        if (playlist.trackIds.isEmpty()) {
            return ToolResult(success = false, error = "Playlist \"${playlist.name}\" is empty")
        }

        player.addUserPlaylistToQueue(playlistId)

        return ToolResult(
            success = true,
            data = "Added playlist \"${playlist.name}\" (${playlist.trackIds.size} tracks) to queue"
        )
    }
}
