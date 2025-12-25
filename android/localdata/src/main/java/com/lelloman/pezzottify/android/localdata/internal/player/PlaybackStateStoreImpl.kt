package com.lelloman.pezzottify.android.localdata.internal.player

import android.content.Context
import android.content.SharedPreferences
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.player.PlaybackStateStore
import com.lelloman.pezzottify.android.domain.player.SavedPlaybackState
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * SharedPreferences-based implementation of PlaybackStateStore.
 *
 * Stores playback state in SharedPreferences for quick access and recovery.
 * State expires after [MAX_STATE_AGE_MS] to avoid restoring stale sessions.
 */
internal class PlaybackStateStoreImpl(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : PlaybackStateStore {

    private val prefs: SharedPreferences = context.getSharedPreferences(
        PREFS_NAME,
        Context.MODE_PRIVATE
    )

    private val json = Json { ignoreUnknownKeys = true }

    override suspend fun saveState(
        playlist: PlaybackPlaylist,
        currentTrackIndex: Int,
        positionMs: Long,
        isPlaying: Boolean
    ) = withContext(dispatcher) {
        val persistable = PersistablePlaybackState(
            context = when (val ctx = playlist.context) {
                is PlaybackPlaylistContext.Album -> PersistableContext.Album(ctx.albumId)
                is PlaybackPlaylistContext.UserPlaylist -> PersistableContext.UserPlaylist(
                    ctx.userPlaylistId,
                    ctx.isEdited
                )
                is PlaybackPlaylistContext.UserMix -> PersistableContext.UserMix
            },
            tracksIds = playlist.tracksIds,
            currentTrackIndex = currentTrackIndex,
            positionMs = positionMs,
            isPlaying = isPlaying,
            savedAtMs = System.currentTimeMillis(),
        )

        prefs.edit()
            .putString(KEY_STATE, json.encodeToString(persistable))
            .apply()
    }

    override suspend fun loadState(): SavedPlaybackState? = withContext(dispatcher) {
        val stateJson = prefs.getString(KEY_STATE, null) ?: return@withContext null

        try {
            val persistable = json.decodeFromString<PersistablePlaybackState>(stateJson)

            // Check if state is too old
            val age = System.currentTimeMillis() - persistable.savedAtMs
            if (age > MAX_STATE_AGE_MS) {
                clearState()
                return@withContext null
            }

            val context = when (val ctx = persistable.context) {
                is PersistableContext.Album -> PlaybackPlaylistContext.Album(ctx.albumId)
                is PersistableContext.UserPlaylist -> PlaybackPlaylistContext.UserPlaylist(
                    ctx.userPlaylistId,
                    ctx.isEdited
                )
                is PersistableContext.UserMix -> PlaybackPlaylistContext.UserMix
            }

            SavedPlaybackState(
                playlist = PlaybackPlaylist(
                    context = context,
                    tracksIds = persistable.tracksIds,
                ),
                currentTrackIndex = persistable.currentTrackIndex,
                positionMs = persistable.positionMs,
                isPlaying = persistable.isPlaying,
                savedAtMs = persistable.savedAtMs,
            )
        } catch (e: Exception) {
            // Corrupted state - clear it
            clearState()
            null
        }
    }

    override suspend fun clearState() = withContext(dispatcher) {
        prefs.edit()
            .remove(KEY_STATE)
            .apply()
    }

    companion object {
        private const val PREFS_NAME = "PlaybackStateStore"
        private const val KEY_STATE = "saved_state"

        // State expires after 24 hours
        private const val MAX_STATE_AGE_MS = 24 * 60 * 60 * 1000L
    }
}

@Serializable
private data class PersistablePlaybackState(
    val context: PersistableContext,
    val tracksIds: List<String>,
    val currentTrackIndex: Int,
    val positionMs: Long,
    val isPlaying: Boolean,
    val savedAtMs: Long,
)

@Serializable
private sealed interface PersistableContext {
    @Serializable
    data class Album(val albumId: String) : PersistableContext

    @Serializable
    data class UserPlaylist(val userPlaylistId: String, val isEdited: Boolean) : PersistableContext

    @Serializable
    data object UserMix : PersistableContext
}
