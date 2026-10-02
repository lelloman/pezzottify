package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import kotlinx.coroutines.flow.first
import javax.inject.Inject

/**
 * Steers the current queue's smart continuation toward an artist, album or track over
 * [stepsTotal] appended tracks. Returns false when the queue cannot be steered (nothing
 * playing or a radio).
 */
class SetPlaybackDestination @Inject constructor(
    private val playbackGravity: PlaybackGravity,
    private val staticsStore: StaticsStore,
) {
    suspend operator fun invoke(
        entityType: String,
        entityId: String,
        stepsTotal: Int,
        label: String? = null,
    ): Boolean {
        if (!playbackGravity.current().isSteerable) return false
        val resolvedLabel = label ?: resolveLabel(entityType, entityId)
        return playbackGravity.update {
            it.steeringToward(GravityReference(entityType, entityId, resolvedLabel), stepsTotal)
        }
    }

    suspend fun resolveLabel(entityType: String, entityId: String): String = runCatching {
        when (entityType) {
            "track" -> staticsStore.getTrack(entityId).first()?.name
            "album" -> staticsStore.getAlbum(entityId).first()?.name
            "artist" -> staticsStore.getArtist(entityId).first()?.name
            else -> null
        }
    }.getOrNull() ?: entityId
}
