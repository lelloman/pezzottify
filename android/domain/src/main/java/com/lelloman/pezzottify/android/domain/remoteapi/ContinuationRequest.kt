package com.lelloman.pezzottify.android.domain.remoteapi

import com.lelloman.pezzottify.android.domain.player.GravityDiagnostics

/**
 * Request for `POST /v1/content/recommendations/continuation`.
 * Null scalars and null lists are omitted on the wire so the server applies its defaults.
 */
data class ContinuationRequest(
    /** Legacy field, still sent so a not-yet-upgraded server keeps working. */
    val contextTrackIds: List<String> = emptyList(),
    val sourceTrackIds: List<String> = emptyList(),
    val sourceReferences: List<ContinuationReference> = emptyList(),
    val recentTrackIds: List<String> = emptyList(),
    val recencyWeight: Double? = null,
    val excludeTrackIds: List<String> = emptyList(),
    val count: Int = 1,
    /** The destination mix, 1..8 components; null when not steering. */
    val destination: List<ContinuationReference>? = null,
    val progress: Double? = null,
    val criteria: List<Criterion>? = null,
    val diversity: Double? = null,
    val randomness: Double? = null,
    val mode: String? = null,
    val away: List<ContinuationReference> = emptyList(),
) {
    data class Criterion(val namespace: String, val weight: Double)
}

data class ContinuationReference(
    val entityType: String,
    val entityId: String,
    val weight: Double? = null,
)

data class ContinuationResult(
    val trackIds: List<String>,
    val diagnostics: GravityDiagnostics?,
)
