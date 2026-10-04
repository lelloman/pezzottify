package com.lelloman.pezzottify.android.ui.screen.steering

enum class SteeringAvailability { Loading, NothingPlaying, Radio, Ready }

enum class SteeringSearchTarget { Destination, Away, Source }

data class SteeringReference(
    val entityType: String,
    val entityId: String,
    val label: String,
    /** Concept family (e.g. `instrument`) when [entityType] is `concept`. */
    val family: String? = null,
    /** Secondary line in search results, e.g. the artists of an album or track. */
    val detail: String? = null,
)

sealed interface SteeringSource {
    data class Queue(
        val userChosenCount: Int,
        val suggestedCount: Int,
        /** First user-chosen tracks, for the artwork collage. */
        val previewTrackIds: List<String> = emptyList(),
    ) : SteeringSource
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

/** Which way of comparing music a namespace stands for, so the UI can name it plainly. */
enum class SteeringCriterionKind { OverallSound, AudioScene, Instruments, Other }

data class SteeringCriterion(
    val namespace: String,
    /** Server label, used only for [SteeringCriterionKind.Other]. */
    val label: String,
    val kind: SteeringCriterionKind,
    val weight: Float,
)

/** The "Along the way" settings. Null values in the stored state show the server defaults. */
data class SteeringKnobs(
    /** 0 = stay on the starting point, 1 = follow the recently played tracks. */
    val recencyWeight: Float = DEFAULT_RECENCY_WEIGHT,
    /** 0 = focused, 1 = varied. Drives both artist/album spreading and pick shuffling. */
    val variety: Float = DEFAULT_VARIETY,
    val criteria: List<SteeringCriterion> = emptyList(),
    val away: List<SteeringReference> = emptyList(),
    val isDefault: Boolean = true,
) {
    companion object {
        // Mirror the server defaults so an untouched setting shows the value actually used.
        const val DEFAULT_RECENCY_WEIGHT = 0.2f
        const val DEFAULT_VARIETY = 0.3f
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
) {
    /** The catalog is only searched from [MIN_SEARCH_QUERY_LENGTH] characters. */
    val isQueryTooShort: Boolean
        get() = query.isNotBlank() && query.trim().length < MIN_SEARCH_QUERY_LENGTH
}

/** Shorter queries are very expensive on a large catalog index and rarely useful. */
const val MIN_SEARCH_QUERY_LENGTH = 2

data class SteeringScreenState(
    val availability: SteeringAvailability = SteeringAvailability.Loading,
    val isRemote: Boolean = false,
    val smartContinuationEnabled: Boolean = false,
    val source: SteeringSource = SteeringSource.Queue(0, 0),
    val destination: SteeringDestination? = null,
    val stepsTotal: Int = 10,
    val stepsDone: Int = 0,
    val knobs: SteeringKnobs = SteeringKnobs(),
    val search: SteeringSearch? = null,
) {
    val stepsRemaining: Int get() = (stepsTotal - stepsDone).coerceAtLeast(0)
}
