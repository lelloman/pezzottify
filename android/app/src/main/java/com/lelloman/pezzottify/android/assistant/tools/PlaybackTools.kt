package com.lelloman.pezzottify.android.assistant.tools

import com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.simpleaiassistant.tool.Tool
import com.lelloman.simpleaiassistant.tool.ToolResult
import com.lelloman.simpleaiassistant.tool.ToolSpec
import kotlinx.coroutines.flow.first

/**
 * Tool to control playback (play/pause/stop).
 */
class PlaybackControlTool(
    private val player: PezzottifyPlayer
) : Tool {
    override val spec = ToolSpec(
        name = "playback_control",
        description = "Control music playback. Use this to play, pause, or stop the music.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "action" to mapOf(
                    "type" to "string",
                    "enum" to listOf("play", "pause", "toggle", "stop"),
                    "description" to "The playback action to perform"
                )
            ),
            "required" to listOf("action")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val action = input["action"] as? String
            ?: return ToolResult(success = false, error = "Missing action parameter")

        return when (action) {
            "play" -> {
                player.setIsPlaying(true)
                ToolResult(success = true, data = "Started playback")
            }
            "pause" -> {
                player.setIsPlaying(false)
                ToolResult(success = true, data = "Paused playback")
            }
            "toggle" -> {
                player.togglePlayPause()
                val isPlaying = player.isPlaying.first()
                ToolResult(success = true, data = if (isPlaying) "Started playback" else "Paused playback")
            }
            "stop" -> {
                player.stop()
                ToolResult(success = true, data = "Stopped playback")
            }
            else -> ToolResult(success = false, error = "Unknown action: $action")
        }
    }
}

/**
 * Tool to skip to next or previous track.
 */
class SkipTrackTool(
    private val player: PezzottifyPlayer
) : Tool {
    override val spec = ToolSpec(
        name = "skip_track",
        description = "Skip to the next or previous track in the queue.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "direction" to mapOf(
                    "type" to "string",
                    "enum" to listOf("next", "previous"),
                    "description" to "Skip direction"
                )
            ),
            "required" to listOf("direction")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val direction = input["direction"] as? String
            ?: return ToolResult(success = false, error = "Missing direction parameter")

        return when (direction) {
            "next" -> {
                player.skipToNextTrack()
                ToolResult(success = true, data = "Skipped to next track")
            }
            "previous" -> {
                player.skipToPreviousTrack()
                ToolResult(success = true, data = "Skipped to previous track")
            }
            else -> ToolResult(success = false, error = "Unknown direction: $direction")
        }
    }
}

/**
 * Tool to control shuffle and repeat modes.
 */
class PlaybackModeTool(
    private val player: PezzottifyPlayer
) : Tool {
    override val spec = ToolSpec(
        name = "playback_mode",
        description = "Control shuffle and repeat modes for playback.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "shuffle" to mapOf(
                    "type" to "boolean",
                    "description" to "Enable or disable shuffle mode"
                ),
                "repeat" to mapOf(
                    "type" to "string",
                    "enum" to listOf("off", "all", "one"),
                    "description" to "Set repeat mode: off, all (repeat queue), or one (repeat current track)"
                )
            )
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val results = mutableListOf<String>()

        // Handle shuffle
        val shuffleInput = input["shuffle"]
        if (shuffleInput != null) {
            val wantShuffle = shuffleInput as? Boolean ?: (shuffleInput.toString().toBoolean())
            val currentShuffle = player.shuffleEnabled.first()
            if (wantShuffle != currentShuffle) {
                player.toggleShuffle()
            }
            results.add("Shuffle ${if (wantShuffle) "enabled" else "disabled"}")
        }

        // Handle repeat
        val repeatInput = input["repeat"] as? String
        if (repeatInput != null) {
            val targetMode = when (repeatInput.lowercase()) {
                "off" -> RepeatMode.OFF
                "all" -> RepeatMode.ALL
                "one" -> RepeatMode.ONE
                else -> null
            }
            if (targetMode != null) {
                // Cycle until we reach the desired mode
                var currentMode = player.repeatMode.first()
                var attempts = 0
                while (currentMode != targetMode && attempts < 3) {
                    player.cycleRepeatMode()
                    currentMode = player.repeatMode.first()
                    attempts++
                }
                results.add("Repeat mode set to $repeatInput")
            }
        }

        return if (results.isNotEmpty()) {
            ToolResult(success = true, data = results.joinToString(". "))
        } else {
            ToolResult(success = false, error = "No valid parameters provided")
        }
    }
}

/**
 * Tool to get current playback status and track info.
 */
class NowPlayingTool(
    private val player: PezzottifyPlayer,
    private val metadataProvider: PlaybackMetadataProvider
) : Tool {
    override val spec = ToolSpec(
        name = "now_playing",
        description = "Get information about the currently playing track, including title, artist, album, and playback status.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf<String, Any>()
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val queueState = metadataProvider.queueState.first()
        val currentTrack = queueState?.currentTrack

        if (currentTrack == null) {
            return ToolResult(success = true, data = "Nothing is currently playing")
        }

        val isPlaying = player.isPlaying.first()
        val progressSec = player.currentTrackProgressSec.first() ?: 0
        val durationSec = currentTrack.durationSeconds
        val shuffleEnabled = player.shuffleEnabled.first()
        val repeatMode = player.repeatMode.first()

        val info = buildString {
            appendLine("Currently ${if (isPlaying) "playing" else "paused"}:")
            appendLine("Track: ${currentTrack.trackName}")
            appendLine("Artist: ${currentTrack.artistNames.joinToString(", ")}")
            appendLine("Album: ${currentTrack.albumName}")
            appendLine("Progress: ${formatTime(progressSec)} / ${formatTime(durationSec)}")
            appendLine("Queue position: ${(queueState.currentIndex + 1)} of ${queueState.tracks.size}")
            appendLine("Shuffle: ${if (shuffleEnabled) "on" else "off"}")
            append("Repeat: ${repeatMode.name.lowercase()}")
        }

        return ToolResult(success = true, data = info)
    }

    private fun formatTime(seconds: Int): String {
        val mins = seconds / 60
        val secs = seconds % 60
        return "%d:%02d".format(mins, secs)
    }
}

/**
 * Tool to view and manage the playback queue.
 */
class QueueTool(
    private val player: PezzottifyPlayer,
    private val metadataProvider: PlaybackMetadataProvider
) : Tool {
    override val spec = ToolSpec(
        name = "queue",
        description = "View the current playback queue or add tracks to it.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "action" to mapOf(
                    "type" to "string",
                    "enum" to listOf("view", "add_track", "clear"),
                    "description" to "Action to perform on the queue"
                ),
                "track_id" to mapOf(
                    "type" to "string",
                    "description" to "Track ID to add (required for add_track action)"
                )
            ),
            "required" to listOf("action")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val action = input["action"] as? String
            ?: return ToolResult(success = false, error = "Missing action parameter")

        return when (action) {
            "view" -> {
                val queueState = metadataProvider.queueState.first()
                if (queueState == null || queueState.tracks.isEmpty()) {
                    return ToolResult(success = true, data = "Queue is empty")
                }

                val info = buildString {
                    appendLine("Queue (${queueState.tracks.size} tracks):")
                    queueState.tracks.forEachIndexed { index, track ->
                        val marker = if (index == queueState.currentIndex) "▶ " else "  "
                        appendLine("$marker${index + 1}. ${track.trackName} - ${track.artistNames.joinToString(", ")}")
                    }
                }
                ToolResult(success = true, data = info.trimEnd())
            }
            "add_track" -> {
                val trackId = input["track_id"] as? String
                    ?: return ToolResult(success = false, error = "Missing track_id parameter")
                player.addTracksToPlaylist(listOf(trackId))
                ToolResult(success = true, data = "Track added to queue")
            }
            "clear" -> {
                player.stop()
                ToolResult(success = true, data = "Queue cleared")
            }
            else -> ToolResult(success = false, error = "Unknown action: $action")
        }
    }
}

/**
 * Tool to play an album.
 */
class PlayAlbumTool(
    private val player: PezzottifyPlayer
) : Tool {
    override val spec = ToolSpec(
        name = "play_album",
        description = "Play an album by its ID. Use the search tool first to find album IDs.",
        inputSchema = mapOf(
            "type" to "object",
            "properties" to mapOf(
                "album_id" to mapOf(
                    "type" to "string",
                    "description" to "The album ID to play"
                ),
                "start_track_id" to mapOf(
                    "type" to "string",
                    "description" to "Optional: Start playing from a specific track"
                )
            ),
            "required" to listOf("album_id")
        )
    )

    override suspend fun execute(input: Map<String, Any?>): ToolResult {
        val albumId = input["album_id"] as? String
            ?: return ToolResult(success = false, error = "Missing album_id parameter")
        val startTrackId = input["start_track_id"] as? String

        player.loadAlbum(albumId, startTrackId)
        return ToolResult(success = true, data = "Started playing album")
    }
}
