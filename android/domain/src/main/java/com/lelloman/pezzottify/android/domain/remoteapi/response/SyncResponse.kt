package com.lelloman.pezzottify.android.domain.remoteapi.response

import com.lelloman.pezzottify.android.domain.notifications.Notification
import com.lelloman.pezzottify.android.domain.sync.Permission
import com.lelloman.pezzottify.android.domain.sync.PermissionListSerializer
import com.lelloman.pezzottify.android.domain.sync.StoredEvent
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.sync.UserSettingListSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Response from GET /v1/sync/state endpoint.
 * Contains the full user state for initial sync.
 */
@Serializable
data class SyncStateResponse(
    val seq: Long,
    val likes: LikesState,
    @Serializable(with = UserSettingListSerializer::class)
    val settings: List<UserSetting>,
    val playlists: List<PlaylistState>,
    @Serializable(with = PermissionListSerializer::class)
    val permissions: List<Permission>,
    val notifications: List<Notification> = emptyList(),
)

/**
 * Liked content state.
 */
@Serializable
data class LikesState(
    val albums: List<String>,
    val artists: List<String>,
    val tracks: List<String>,
)

/**
 * Playlist state.
 */
@Serializable
data class PlaylistState(
    val id: String,
    val name: String,
    val tracks: List<String>,
)

/**
 * Response from GET /v1/sync/events endpoint.
 * Contains events since a given sequence number.
 */
@Serializable
data class SyncEventsResponse(
    val events: List<StoredEvent>,
    @SerialName("current_seq")
    val currentSeq: Long,
)

/**
 * Error response for sync events endpoint.
 * Returned with 410 GONE status when events have been pruned.
 */
@Serializable
data class SyncEventsError(
    val error: String,
    val message: String,
)
