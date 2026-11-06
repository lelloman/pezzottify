package com.lelloman.pezzottify.android.player

import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import androidx.media3.common.util.Util
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.lelloman.pezzottify.android.domain.player.ControlsAndStatePlayer
import com.lelloman.pezzottify.android.domain.player.PlaybackQueueState
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlin.math.abs

internal data class RemoteSessionTrack(
    val id: String,
    val title: String,
    val artists: String,
    val album: String,
    val durationMs: Long,
)

/** What the controlled device is doing, as far as this device knows. */
internal data class RemoteSessionSnapshot(
    val deviceName: String,
    val tracks: List<RemoteSessionTrack>,
    val currentIndex: Int,
    val isPlaying: Boolean,
    val positionMs: Long,
)

/** Everything except the position, which [RemoteSessionPlayer.update] treats separately. */
internal fun remoteSessionSnapshots(
    deviceName: String,
    player: ControlsAndStatePlayer,
    queueState: StateFlow<PlaybackQueueState?>,
): Flow<RemoteSessionSnapshot> = combine(
    queueState,
    player.isPlaying,
    player.currentTrackProgressSec,
) { queue, isPlaying, progressSec ->
    val tracks = queue?.tracks.orEmpty()
    RemoteSessionSnapshot(
        deviceName = deviceName,
        tracks = tracks.map {
            RemoteSessionTrack(
                id = it.trackId,
                title = it.trackName,
                artists = it.artistNames.joinToString(", "),
                album = it.albumName,
                durationMs = it.durationSeconds * 1000L,
            )
        },
        currentIndex = (queue?.currentIndex ?: 0).coerceIn(0, (tracks.size - 1).coerceAtLeast(0)),
        isPlaying = isPlaying,
        positionMs = (progressSec ?: 0) * 1000L,
    )
}

/**
 * Presents the controlled device as a player, so the media session gives remote playback the same
 * notification, lock screen and headset controls as local playback. Nothing here produces sound:
 * state comes in through [update] and controls go out through [controls].
 */
@OptIn(UnstableApi::class)
internal class RemoteSessionPlayer(
    looper: Looper,
    private val controls: ControlsAndStatePlayer,
    deviceName: String,
    private val clockMs: () -> Long = { System.currentTimeMillis() },
) : SimpleBasePlayer(looper) {

    private var snapshot = RemoteSessionSnapshot(deviceName, emptyList(), 0, false, 0L)
    private var snapshotAtMs = clockMs()
    private var artworkTrackId: String? = null
    private var artworkBytes: ByteArray? = null

    fun update(next: RemoteSessionSnapshot) {
        val positionJumped = abs(next.positionMs - expectedPositionMs()) > POSITION_TOLERANCE_MS
        val sameOtherwise = next.copy(positionMs = 0L) == snapshot.copy(positionMs = 0L)
        if (sameOtherwise && !positionJumped) return
        snapshot = next
        snapshotAtMs = clockMs()
        invalidateState()
    }

    fun setArtwork(trackId: String, bytes: ByteArray) {
        artworkTrackId = trackId
        artworkBytes = bytes
        invalidateState()
    }

    private fun expectedPositionMs(): Long =
        snapshot.positionMs + if (snapshot.isPlaying) clockMs() - snapshotAtMs else 0L

    override fun getState(): State {
        val position = snapshot.positionMs
        val builder = State.Builder()
            .setAvailableCommands(AVAILABLE_COMMANDS)
            .setPlayWhenReady(snapshot.isPlaying, Player.PLAY_WHEN_READY_CHANGE_REASON_REMOTE)
            .setPlaybackState(Player.STATE_READY)
            .setPlaylist(playlist())
            .setCurrentMediaItemIndex(snapshot.currentIndex)
        return if (snapshot.isPlaying) {
            builder.setContentPositionMs(PositionSupplier.getExtrapolating(position, 1f))
        } else {
            builder.setContentPositionMs(position)
        }.build()
    }

    /** Never empty: the service only stays in the foreground while there is an item to show. */
    private fun playlist(): List<MediaItemData> {
        if (snapshot.tracks.isEmpty()) {
            return listOf(itemData("placeholder", null, MediaMetadata.Builder().setTitle(snapshot.deviceName)))
        }
        return snapshot.tracks.mapIndexed { index, track ->
            val metadata = MediaMetadata.Builder()
                .setTitle(track.title)
                .setArtist(listOf(track.artists, snapshot.deviceName).filter { it.isNotBlank() }.joinToString(" · "))
                .setAlbumTitle(track.album)
            if (track.id == artworkTrackId) {
                metadata.setArtworkData(artworkBytes, MediaMetadata.PICTURE_TYPE_FRONT_COVER)
            }
            // The same track can be queued twice, so the id alone is not unique.
            itemData("$index:${track.id}", track, metadata)
        }
    }

    private fun itemData(uid: String, track: RemoteSessionTrack?, metadata: MediaMetadata.Builder): MediaItemData {
        val mediaMetadata = metadata.build()
        val item = MediaItemData.Builder(uid)
            .setMediaItem(MediaItem.Builder().setMediaId(uid).setMediaMetadata(mediaMetadata).build())
            .setMediaMetadata(mediaMetadata)
            .setIsSeekable(track != null)
        if (track != null && track.durationMs > 0) item.setDurationUs(Util.msToUs(track.durationMs))
        return item.build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        controls.setIsPlaying(playWhenReady)
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> controls.skipToNextTrack()
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> controls.skipToPreviousTrack()
            Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM -> {
                val durationMs = snapshot.tracks.getOrNull(snapshot.currentIndex)?.durationMs ?: 0L
                if (durationMs > 0) controls.seekToPercentage(positionMs * 100f / durationMs)
            }
            else -> controls.loadTrackIndex(mediaItemIndex)
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        controls.stop()
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> = Futures.immediateVoidFuture()

    private companion object {
        /** The remote device reports every few seconds, so smaller gaps are just noise. */
        const val POSITION_TOLERANCE_MS = 2_000L

        val AVAILABLE_COMMANDS: Player.Commands = Player.Commands.Builder()
            .addAll(
                Player.COMMAND_PLAY_PAUSE,
                Player.COMMAND_STOP,
                Player.COMMAND_RELEASE,
                Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_NEXT,
                Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_PREVIOUS,
                Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_MEDIA_ITEM,
                Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
                Player.COMMAND_GET_TIMELINE,
                Player.COMMAND_GET_METADATA,
            )
            .build()
    }
}
