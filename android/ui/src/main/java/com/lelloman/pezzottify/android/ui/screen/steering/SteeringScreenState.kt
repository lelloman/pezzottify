package com.lelloman.pezzottify.android.ui.screen.steering

enum class SteeringAvailability { Loading, NothingPlaying, Radio, Ready }

enum class SteeringSearchTarget { Destination, Away, Source }

data class SteeringReference(
    val entityType: String,
    val entityId: String,
    val label: String,
)

sealed interface SteeringSource {
    data class Queue(val userChosenCount: Int, val suggestedCount: Int) : SteeringSource
    data class References(val references: List<SteeringReference>) : SteeringSource
}

data class SteeringDestination(
    val reference: SteeringReference,
    val progress: Float,
    /** Cosine similarity of the last continuation query to the destination, if known. */
    val queryToDestination: Float?,
    /** Cosine similarity of the source to the destination, if known. */
    val sourceToDestination: Float?,
)

data class SteeringCriterion(
    val namespace: String,
    val label: String,
    val weight: Float,
)

data class SteeringKnobs(
    val recencyWeight: Float = DEFAULT_RECENCY_WEIGHT,
    val diversity: Float = DEFAULT_DIVERSITY,
    val randomness: Float = DEFAULT_RANDOMNESS,
    val mode: String = MODE_SIMILAR,
    val criteria: List<SteeringCriterion> = emptyList(),
    val away: List<SteeringReference> = emptyList(),
    val isDefault: Boolean = true,
) {
    companion object {
        // Mirror the server defaults so an untouched knob shows the value actually used.
        const val DEFAULT_RECENCY_WEIGHT = 0.2f
        const val DEFAULT_DIVERSITY = 0.3f
        const val DEFAULT_RANDOMNESS = 0.3f
        const val MODE_SIMILAR = "similar"
        const val MODE_EXPLORE = "explore"
    }
}

data class SteeringSearch(
    val target: SteeringSearchTarget,
    val query: String = "",
    val isSearching: Boolean = false,
    val results: List<SteeringReference> = emptyList(),
    val isError: Boolean = false,
)

data class SteeringScreenState(
    val availability: SteeringAvailability = SteeringAvailability.Loading,
    val isRemote: Boolean = false,
    val smartContinuationEnabled: Boolean = false,
    val source: SteeringSource = SteeringSource.Queue(0, 0),
    val destination: SteeringDestination? = null,
    val stepsTotal: Int = 20,
    val stepsDone: Int = 0,
    val knobs: SteeringKnobs = SteeringKnobs(),
    val search: SteeringSearch? = null,
) {
    val stepsRemaining: Int get() = (stepsTotal - stepsDone).coerceAtLeast(0)

    /** Explore mode deliberately avoids the closest matches, so it never arrives. */
    val canUseExplore: Boolean get() = destination == null
}
