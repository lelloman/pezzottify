package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.Serializable

/**
 * Request to fetch multiple content items in a single batch.
 * Server limit: 100 items total across all types.
 */
@Serializable
data class BatchContentRequest(
    val artists: List<BatchItemRequest> = emptyList(),
    val albums: List<BatchItemRequest> = emptyList(),
    val tracks: List<BatchItemRequest> = emptyList(),
)

/**
 * Individual item in a batch request.
 * @param id The ID of the item to fetch
 * @param resolved If true, returns ResolvedArtist/ResolvedAlbum/ResolvedTrack.
 *                 If false, returns basic Artist/Album/Track.
 */
@Serializable
data class BatchItemRequest(
    val id: String,
    val resolved: Boolean = true,
)
