package com.lelloman.pezzottify.android.domain.playbacksession

data class ConnectedDevice(
    val id: Int,
    val name: String,
    val deviceType: String,
)

data class RemotePlaybackState(
    val currentTrack: RemoteTrackInfo?,
    val position: Double,
    val isPlaying: Boolean,
    val volume: Float,
    val muted: Boolean,
    val shuffle: Boolean,
    val repeat: String,
    val timestamp: Long,
    /** Local clock time when this state was received, for clock-skew-safe interpolation. */
    val receivedAt: Long = 0L,
)

data class RemoteTrackInfo(
    val id: String,
    val title: String,
    val artistName: String?,
    val albumTitle: String?,
    val durationMs: Long,
    val imageId: String?,
)
