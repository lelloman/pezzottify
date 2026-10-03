package com.lelloman.pezzottify.android.ui.screen.steering

enum class SteeringAvailability { Loading, NothingPlaying, Radio, Ready }

enum class SteeringSearchTarget { Destination, Away, Source }

data class SteeringReference(
    val entityType: String,
    val entityId: String,
    val label: String,
    /** Concept family (e.g. `instrument`) when [entityType] is `concept`. */
    val family: String? = null,
)

sealed interface SteeringSource {
    data class Queue(val userChosenCount: Int, val suggestedCount: Int) : SteeringSource
    data class References(val references: List<SteeringReference>) : SteeringSource
}

data class SteeringDestinationComponent(
    val reference: SteeringReference,
    val weight: Float,
    /** Cosine similarity of the last continuation query to this component, if known. */
    val similarity: Float?,
)

/** The destination mix: 1..[SteeringScreenViewModel.MAX_REFERENCES] weighted components. */
data class SteeringDestination(
    val components: List<SteeringDestinationComponent>,
    val progress: Float,
    /** Cosine similarity of the last continuation query to the whole mix, if known. */
    val queryToDestination: Float?,
    /** Cosine similarity of the source to the whole mix, if known. */
    val sourceToDestination: Float?,
) {
    val isFull: Boolean get() = components.size >= SteeringScreenViewModel.MAX_REFERENCES
}

/** Concepts of one family, in server order. */
data class SteeringConceptGroup(
    val family: String,
    val concepts: List<SteeringReference>,
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
    /** Concepts matching [query] (all of them when it is blank), grouped by family. */
    val concepts: List<SteeringConceptGroup> = emptyList(),
    val isLoadingConcepts: Boolean = false,
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
