package com.lelloman.pezzottify.android.domain.playback

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Device information for remote playback.
 */
@Serializable
data class PlaybackDevice(
    val id: Long,
    val name: String,
    @SerialName("device_type")
    val deviceType: String,
    @SerialName("is_audio_device")
    val isAudioDevice: Boolean = false,
)

/**
 * Track information in playback state.
 */
@Serializable
data class PlaybackTrack(
    val id: String,
    val title: String,
    @SerialName("artist_id")
    val artistId: String = "",
    @SerialName("artist_name")
    val artistName: String = "Unknown Artist",
    @SerialName("album_id")
    val albumId: String = "",
    @SerialName("album_title")
    val albumTitle: String = "Unknown Album",
    val duration: Float = 0f,
    @SerialName("track_number")
    val trackNumber: Int? = null,
    @SerialName("image_id")
    val imageId: String? = null,
)

/**
 * Queue item for playback queue.
 */
@Serializable
data class QueueItem(
    val id: String,
    @SerialName("added_at")
    val addedAt: Long,
)

/**
 * Playback state broadcast by audio device.
 */
@Serializable
data class PlaybackState(
    @SerialName("current_track")
    val currentTrack: PlaybackTrack? = null,
    @SerialName("queue_position")
    val queuePosition: Int = 0,
    @SerialName("queue_version")
    val queueVersion: Int = 0,
    val position: Float = 0f,
    @SerialName("is_playing")
    val isPlaying: Boolean = false,
    val volume: Float = 1f,
    val muted: Boolean = false,
    val shuffle: Boolean = false,
    val repeat: String = "off",
    val timestamp: Long = System.currentTimeMillis(),
)

// ============================================
// Message payloads
// ============================================

/**
 * Hello message payload (client -> server).
 */
@Serializable
data class HelloPayload(
    @SerialName("device_name")
    val deviceName: String,
    @SerialName("device_type")
    val deviceType: String,
)

/**
 * Session info in welcome message.
 */
@Serializable
data class SessionInfo(
    val exists: Boolean = false,
    val reclaimable: Boolean = false,
    val state: PlaybackState? = null,
    val queue: List<QueueItem>? = null,
)

/**
 * Welcome message payload (server -> client).
 */
@Serializable
data class WelcomePayload(
    @SerialName("device_id")
    val deviceId: Long,
    val devices: List<PlaybackDevice>,
    val session: SessionInfo,
)

/**
 * Device list changed payload.
 */
@Serializable
data class DeviceChange(
    val type: String, // "joined" or "left"
    @SerialName("device_id")
    val deviceId: Long,
)

@Serializable
data class DeviceListChangedPayload(
    val devices: List<PlaybackDevice>,
    val change: DeviceChange,
)

/**
 * Command payload (controller -> audio device via server).
 */
@Serializable
data class CommandPayload(
    val command: String,
    val payload: CommandData? = null,
)

/**
 * Command-specific data.
 */
@Serializable
data class CommandData(
    val position: Float? = null,
    val volume: Float? = null,
    val muted: Boolean? = null,
    @SerialName("transfer_id")
    val transferId: String? = null,
)

/**
 * Queue sync/update payload.
 */
@Serializable
data class QueuePayload(
    val queue: List<QueueItem>,
    @SerialName("queue_version")
    val queueVersion: Int,
)

/**
 * Session ended payload.
 */
@Serializable
data class SessionEndedPayload(
    val reason: String? = null,
)

/**
 * Prepare transfer payload (server -> source audio device).
 */
@Serializable
data class PrepareTransferPayload(
    @SerialName("transfer_id")
    val transferId: String,
    @SerialName("target_device_id")
    val targetDeviceId: String,
    @SerialName("target_device_name")
    val targetDeviceName: String,
)

/**
 * Transfer ready payload (source -> server).
 */
@Serializable
data class TransferReadyPayload(
    @SerialName("transfer_id")
    val transferId: String,
    val state: PlaybackState,
    val queue: List<QueueItem>,
)

/**
 * Become audio device payload (server -> target).
 */
@Serializable
data class BecomeAudioDevicePayload(
    @SerialName("transfer_id")
    val transferId: String,
    val state: PlaybackState,
    val queue: List<QueueItem>,
)

/**
 * Transfer complete payload.
 */
@Serializable
data class TransferCompletePayload(
    @SerialName("transfer_id")
    val transferId: String,
)

/**
 * Transfer aborted payload.
 */
@Serializable
data class TransferAbortedPayload(
    @SerialName("transfer_id")
    val transferId: String,
    val reason: String,
)
