package com.lelloman.pezzottify.android.domain.playbacksession

data class ConnectedDevice(
    val id: Int,
    val name: String,
    val deviceType: String,
    /** Last reported `is_playing` of the device, as relayed by the server. */
    val isPlaying: Boolean = false,
    /** True when the device has reported a state and has not stopped. */
    val hasSession: Boolean = false,
    /** True when the device belongs to another user who shared it with us. */
    val isShared: Boolean = false,
    val ownerHandle: String? = null,
)

data class RemotePlaybackState(
    val currentTrack: RemoteTrackInfo?,
    /** Position in seconds. */
    val position: Double,
    val isPlaying: Boolean,
    val volume: Float,
    val muted: Boolean,
    val shuffle: Boolean,
    val repeat: String,
    val timestamp: Long,
    /** Local clock time when this state was received, for clock-skew-safe interpolation. */
    val receivedAt: Long = 0L,
    /** Index of the current track within the queue, as reported by the remote device. */
    val queuePosition: Int = 0,
)

data class RemoteTrackInfo(
    val id: String,
    val title: String,
    val artistName: String?,
    val albumTitle: String?,
    /** Duration in milliseconds. On the wire `duration` is seconds; see docs/multi-device-playback.md. */
    val durationMs: Long,
    val imageId: String?,
    val artistId: String? = null,
    val albumId: String? = null,
    val artistsIds: List<String> = emptyList(),
    val trackNumber: Int? = null,
)

/** A `playback.error` reply from the server, surfaced to the UI. */
data class PlaybackSessionError(
    val code: String,
    val message: String,
    val command: String? = null,
)
