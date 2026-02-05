package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Reference to an entity with id and name.
 * Used in What's New summaries for added/deleted items.
 */
@Serializable
data class EntityRef(
    val id: String,
    val name: String,
)

/**
 * Summary of changes for artists, albums, or images.
 * Lists individual names for added/deleted items, counts for updates.
 */
@Serializable
data class EntityChangeSummary(
    val added: List<EntityRef> = emptyList(),
    @SerialName("updated_count")
    val updatedCount: Int = 0,
    val deleted: List<EntityRef> = emptyList(),
)

/**
 * Summary of track changes (counts only due to volume).
 */
@Serializable
data class TrackChangeSummary(
    @SerialName("added_count")
    val addedCount: Int = 0,
    @SerialName("updated_count")
    val updatedCount: Int = 0,
    @SerialName("deleted_count")
    val deletedCount: Int = 0,
)

/**
 * Summary of all changes in a batch, organized by entity type.
 */
@Serializable
data class BatchChangeSummary(
    val artists: EntityChangeSummary = EntityChangeSummary(),
    val albums: EntityChangeSummary = EntityChangeSummary(),
    val tracks: TrackChangeSummary = TrackChangeSummary(),
    val images: EntityChangeSummary = EntityChangeSummary(),
)

/**
 * A batch in the "What's New" response.
 * Represents a closed batch of catalog changes.
 */
@Serializable
data class WhatsNewBatch(
    val id: String,
    val name: String? = null,
    val description: String? = null,
    @SerialName("closed_at")
    val closedAt: Long,
    val summary: BatchChangeSummary,
)

/**
 * Response from /v1/content/whatsnew endpoint.
 * Contains recent catalog update batches with summaries.
 */
@Serializable
data class WhatsNewResponse(
    val batches: List<WhatsNewBatch>,
)
