package com.lelloman.pezzottify.android.domain.remoteapi.response

/**
 * Response from batch content endpoint.
 * Maps content IDs to their fetch results.
 */
data class BatchContentResponse(
    val artists: Map<String, BatchItemResult<ArtistResponse>>,
    val albums: Map<String, BatchItemResult<AlbumResponse>>,
    val tracks: Map<String, BatchItemResult<TrackResponse>>,
)

/**
 * Result for a single item in a batch response.
 * Either Ok with the data or Error with a message.
 */
sealed class BatchItemResult<out T> {
    data class Ok<T>(val value: T) : BatchItemResult<T>()
    data class Error(val error: String) : BatchItemResult<Nothing>()
}
