package com.lelloman.pezzottify.android.ui.screen.steering

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.player.Gravity
import com.lelloman.pezzottify.android.domain.player.GravityCriterion
import com.lelloman.pezzottify.android.domain.player.GravityKnobs
import com.lelloman.pezzottify.android.domain.player.GravityReference
import com.lelloman.pezzottify.android.domain.player.GravitySource
import com.lelloman.pezzottify.android.domain.player.PlaybackGravityState
import com.lelloman.pezzottify.android.domain.player.steeringAlsoToward
import com.lelloman.pezzottify.android.domain.remoteapi.response.Concept
import com.lelloman.pezzottify.android.domain.remoteapi.response.RadioOptions
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SteeringScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), SteeringScreenActions {

    private val radioOptions = MutableStateFlow<RadioOptions?>(null)
    private val search = MutableStateFlow<SteeringSearch?>(null)
    private var searchJob: Job? = null

    /** Loaded once, on first search, and filtered locally: the whole list is a few hundred items. */
    private var concepts: List<Concept>? = null

    val state: StateFlow<SteeringScreenState> = combine(
        interactor.getGravityState(),
        interactor.getSmartContinuationEnabled(),
        radioOptions,
        search,
    ) { gravityState, smartEnabled, options, search ->
        toState(gravityState, smartEnabled, options, search)
    }.stateIn(viewModelScope, SharingStarted.Eagerly, SteeringScreenState())

    init {
        viewModelScope.launch { radioOptions.value = interactor.getRadioOptions() }
    }

    override fun setSmartContinuationEnabled(enabled: Boolean) {
        viewModelScope.launch { interactor.setSmartContinuationEnabled(enabled) }
    }

    override fun setStepsRemaining(remaining: Int) {
        interactor.updateGravity { it.withStepsTotal(it.stepsDone + remaining.coerceAtLeast(1)) }
    }

    override fun clearDestination() {
        interactor.updateGravity { it.withDestination(null) }
    }

    override fun setDestinationComponentWeight(reference: SteeringReference, weight: Float) {
        interactor.updateGravity {
            it.withDestinationComponentWeight(reference.entityType, reference.entityId, weight.toDouble())
        }
    }

    override fun removeDestinationComponent(reference: SteeringReference) {
        interactor.updateGravity { it.withDestinationComponentRemoved(reference.entityType, reference.entityId) }
    }

    override fun resetSourceToQueue() {
        interactor.updateGravity { it.withSource(GravitySource()) }
    }

    override fun removeSourceReference(reference: SteeringReference) {
        interactor.updateGravity { gravity ->
            val remaining = gravity.source.references.filterNot { it.matches(reference) }
            gravity.withSource(
                if (remaining.isEmpty()) GravitySource()
                else GravitySource(kind = GravitySource.KIND_REFERENCES, references = remaining)
            )
        }
    }

    override fun setRecencyWeight(value: Float) = updateKnobs { it.copy(recencyWeight = value.toDouble()) }

    /** One plain "Variety" setting drives both spreading artists/albums and shuffling picks. */
    override fun setVariety(value: Float) = updateKnobs {
        val variety = value.toDouble().coerceIn(0.0, 1.0)
        it.copy(diversity = variety, randomness = variety)
    }

    override fun setCriterionWeight(namespace: String, weight: Float) = updateKnobs { knobs ->
        // Materialise the effective criteria first so untouched namespaces keep their weight.
        val current = state.value.knobs.criteria.associate { it.namespace to it.weight.toDouble() }
        val updated = (current + (namespace to weight.toDouble().coerceAtLeast(0.0)))
            .filterValues { it > 0.0 }
            .map { (ns, w) -> GravityCriterion(ns, w) }
        knobs.copy(criteria = updated.ifEmpty { null })
    }

    override fun removeAway(reference: SteeringReference) = updateKnobs { knobs ->
        knobs.copy(away = knobs.away.filterNot { it.matches(reference) })
    }

    override fun resetKnobs() {
        interactor.updateGravity { it.withKnobs(GravityKnobs()) }
    }

    override fun openSearch(target: SteeringSearchTarget) {
        searchJob?.cancel()
        val loaded = concepts
        search.value = SteeringSearch(
            target = target,
            concepts = loaded?.let { groupConcepts(it, "") }.orEmpty(),
            isLoadingConcepts = loaded == null,
        )
        if (loaded == null) {
            viewModelScope.launch {
                val fetched = interactor.getConcepts()
                if (fetched != null) concepts = fetched
                search.value = search.value?.let {
                    it.copy(isLoadingConcepts = false, concepts = groupConcepts(fetched.orEmpty(), it.query))
                }
            }
        }
    }

    override fun updateSearchQuery(query: String) {
        val current = search.value ?: return
        val grouped = groupConcepts(concepts.orEmpty(), query)
        search.value = current.copy(query = query, isError = false, concepts = grouped)
        searchJob?.cancel()
        if (query.isBlank()) {
            search.value = current.copy(query = query, results = emptyList(), isSearching = false, concepts = grouped)
            return
        }
        searchJob = viewModelScope.launch {
            delay(SEARCH_DEBOUNCE_MS)
            search.value = search.value?.copy(isSearching = true)
            val results = interactor.searchReferences(query)
            search.value = search.value?.copy(
                isSearching = false,
                results = results.orEmpty(),
                isError = results == null,
            )
        }
    }

    override fun pickSearchResult(reference: SteeringReference) {
        val target = search.value?.target ?: return
        closeSearch()
        val gravityReference = reference.toGravityReference()
        when (target) {
            // Picking for the destination grows the mix; a full mix is left unchanged.
            SteeringSearchTarget.Destination -> interactor.updateGravity { it.steeringAlsoToward(gravityReference) }

            SteeringSearchTarget.Away -> updateKnobs { knobs ->
                if (knobs.away.any { it.matches(reference) } || knobs.away.size >= MAX_REFERENCES) knobs
                else knobs.copy(away = knobs.away + gravityReference)
            }

            SteeringSearchTarget.Source -> interactor.updateGravity { gravity ->
                val existing = if (gravity.source.kind == GravitySource.KIND_REFERENCES) {
                    gravity.source.references
                } else {
                    emptyList()
                }
                if (existing.any { it.matches(reference) } || existing.size >= MAX_REFERENCES) gravity
                else gravity.withSource(
                    GravitySource(kind = GravitySource.KIND_REFERENCES, references = existing + gravityReference)
                )
            }
        }
    }

    override fun closeSearch() {
        searchJob?.cancel()
        search.value = null
    }

    /**
     * The screen no longer offers explore mode (it avoids the closest matches, so a journey
     * never arrives); any write from here clears it.
     */
    private fun updateKnobs(transform: (GravityKnobs) -> GravityKnobs) {
        interactor.updateGravity { it.withKnobs(transform(it.knobs).copy(mode = null)) }
    }

    private fun toState(
        gravityState: PlaybackGravityState,
        smartEnabled: Boolean,
        options: RadioOptions?,
        search: SteeringSearch?,
    ): SteeringScreenState {
        val gravity = gravityState.gravity
        val availability = when {
            !gravityState.hasPlaylist -> SteeringAvailability.NothingPlaying
            gravityState.isRadio -> SteeringAvailability.Radio
            gravity == null -> SteeringAvailability.NothingPlaying
            else -> SteeringAvailability.Ready
        }
        if (gravity == null) {
            return SteeringScreenState(
                availability = availability,
                isRemote = gravityState.isRemote,
                smartContinuationEnabled = smartEnabled,
                search = search,
            )
        }
        val auto = gravity.autoTrackIds.toHashSet()
        val source = if (gravity.source.kind == GravitySource.KIND_REFERENCES) {
            SteeringSource.References(gravity.source.references.map { it.toUi() })
        } else {
            val suggested = gravityState.trackIds.count { it in auto }
            SteeringSource.Queue(
                userChosenCount = gravityState.trackIds.size - suggested,
                suggestedCount = suggested,
            )
        }
        val firstNamespace = gravity.lastDiagnostics?.namespaces?.firstOrNull()
        val componentSimilarity = firstNamespace?.destinationComponents
            ?.associate { "${it.entityType}:${it.entityId}" to it.similarity }
            .orEmpty()
        val destination = gravity.destination?.let { components ->
            SteeringDestination(
                components = components.map {
                    SteeringDestinationComponent(
                        reference = it.toUi(),
                        weight = it.weight.toFloat(),
                        similarity = componentSimilarity[it.key]?.toFloat(),
                    )
                },
                progress = gravity.progress().toFloat(),
                queryToDestination = firstNamespace?.queryToDestination?.toFloat(),
                sourceToDestination = firstNamespace?.sourceToDestination?.toFloat(),
            )
        }
        return SteeringScreenState(
            availability = availability,
            isRemote = gravityState.isRemote,
            smartContinuationEnabled = smartEnabled,
            source = source,
            destination = destination,
            stepsTotal = gravity.stepsTotal,
            stepsDone = gravity.stepsDone,
            knobs = knobsUi(gravity, options),
            search = search,
        )
    }

    private fun knobsUi(gravity: Gravity, options: RadioOptions?): SteeringKnobs {
        val knobs = gravity.knobs
        val labels = options?.criteria?.associate { it.namespace to it.label }.orEmpty()
        val selected = knobs.criteria?.associate { it.namespace to it.weight }
        val namespaces = (labels.keys + selected?.keys.orEmpty()).ifEmpty { setOf(DEFAULT_NAMESPACE) }
        val criteria = namespaces.map { namespace ->
            SteeringCriterion(
                namespace = namespace,
                label = labels[namespace] ?: namespace,
                kind = criterionKind(namespace),
                weight = when {
                    selected != null -> selected[namespace]?.toFloat() ?: 0f
                    namespace == DEFAULT_NAMESPACE -> 1f
                    else -> 0f
                },
            )
        }
        val diversity = knobs.diversity?.toFloat()
            ?: options?.diversity?.default?.toFloat() ?: SteeringKnobs.DEFAULT_VARIETY
        val randomness = knobs.randomness?.toFloat()
            ?: options?.randomness?.default?.toFloat() ?: SteeringKnobs.DEFAULT_VARIETY
        return SteeringKnobs(
            recencyWeight = knobs.recencyWeight?.toFloat() ?: SteeringKnobs.DEFAULT_RECENCY_WEIGHT,
            variety = ((diversity + randomness) / 2f).coerceIn(0f, 1f),
            criteria = criteria,
            away = knobs.away.map { it.toUi() },
            isDefault = knobs.copy(mode = null) == GravityKnobs(),
        )
    }

    interface Interactor {
        fun getGravityState(): Flow<PlaybackGravityState>
        fun getSmartContinuationEnabled(): Flow<Boolean>
        suspend fun setSmartContinuationEnabled(enabled: Boolean)
        fun updateGravity(transform: (Gravity) -> Gravity)
        suspend fun getRadioOptions(): RadioOptions?

        /** Artists, albums and tracks matching [query]; null when the search failed. */
        suspend fun searchReferences(query: String): List<SteeringReference>?

        /** Every steering concept; null when loading failed. */
        suspend fun getConcepts(): List<Concept>?
    }

    companion object {
        const val DEFAULT_NAMESPACE = "musicfm.mean.v1"
        const val MAX_REFERENCES = 8
        private const val SEARCH_DEBOUNCE_MS = 300L
    }
}

internal fun criterionKind(namespace: String): SteeringCriterionKind = when {
    namespace.startsWith("musicfm.") -> SteeringCriterionKind.OverallSound
    namespace.startsWith("ast.audioset.") -> SteeringCriterionKind.AudioScene
    namespace.startsWith("ast.instruments.") -> SteeringCriterionKind.Instruments
    else -> SteeringCriterionKind.Other
}

private fun GravityReference.toUi() = SteeringReference(
    entityType = entityType,
    entityId = entityId,
    label = label ?: entityId,
    family = if (entityType == GravityReference.TYPE_CONCEPT) conceptFamily(entityId) else null,
)

private fun SteeringReference.toGravityReference() = GravityReference(entityType, entityId, label)

/** Concept ids are `prefix:value`; AudioSet ids do not encode their family, the server labels do. */
private fun conceptFamily(conceptId: String): String? = when (conceptId.substringBefore(':')) {
    "genre" -> Concept.FAMILY_GENRE_TAG
    "recorded" -> Concept.FAMILY_RECORDED
    "composed" -> Concept.FAMILY_COMPOSED
    else -> null
}

internal fun groupConcepts(concepts: List<Concept>, query: String): List<SteeringConceptGroup> {
    val needle = query.trim()
    return concepts
        .filter { needle.isEmpty() || it.label.contains(needle, ignoreCase = true) }
        .groupBy { it.family }
        .entries
        .sortedBy { (family, _) ->
            Concept.FAMILY_ORDER.indexOf(family).let { if (it < 0) Int.MAX_VALUE else it }
        }
        .map { (family, items) ->
            SteeringConceptGroup(
                family = family,
                concepts = items.map {
                    SteeringReference(GravityReference.TYPE_CONCEPT, it.id, it.label, family)
                },
            )
        }
}

private fun GravityReference.matches(reference: SteeringReference) =
    entityType == reference.entityType && entityId == reference.entityId
