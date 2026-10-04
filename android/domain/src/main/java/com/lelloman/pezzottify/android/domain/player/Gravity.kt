package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.remoteapi.ContinuationReference
import com.lelloman.pezzottify.android.domain.remoteapi.ContinuationRequest
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject

/**
 * Smart-continuation "gravity" state of a playback playlist.
 *
 * The Source centre of gravity is the set of user-chosen tracks (queue minus [autoTrackIds]) or,
 * after arrival at a destination, an explicit list of references. The optional Destination is a
 * weighted mix of 1..[MAX_COMPONENTS] catalog items or concepts that pulls the continuation query
 * toward it over [stepsTotal] auto-appended tracks.
 *
 * This object travels verbatim inside `context.gravity` on the playback-session WebSocket and is
 * persisted with the playlist, so every field has a default and the wire keys are snake_case.
 */
@Serializable
data class Gravity(
    val v: Int = VERSION,
    val source: GravitySource = GravitySource(),
    @SerialName("auto_track_ids") val autoTrackIds: List<String> = emptyList(),
    // v1 stored a single object here; it is not migrated and reads as no destination.
    @Serializable(with = DestinationMixSerializer::class)
    val destination: List<GravityReference>? = null,
    @SerialName("steps_total") val stepsTotal: Int = DEFAULT_STEPS_TOTAL,
    @SerialName("steps_done") val stepsDone: Int = 0,
    val knobs: GravityKnobs = GravityKnobs(),
    @SerialName("last_diagnostics") val lastDiagnostics: GravityDiagnostics? = null,
) {
    // Include defaults: the web client consumes this as plain JSON, not a Kotlin data class.
    fun toWireJson(): JsonObject = wireJson.encodeToJsonElement(serializer(), this).jsonObject

    fun isAuto(trackId: String): Boolean = trackId in autoTrackIds

    /** 0..1 progress toward the destination, 0 when there is none. */
    fun progress(): Double =
        if (destination == null || stepsTotal <= 0) 0.0 else minOf(1.0, stepsDone.toDouble() / stepsTotal)

    fun appendedAuto(trackIds: List<String>): Gravity {
        val auto = (autoTrackIds + trackIds).distinct().takeLast(AUTO_IDS_CAP)
        val next = copy(autoTrackIds = auto)
        if (destination == null) return next
        val done = stepsDone + trackIds.size
        return if (done >= stepsTotal) next.copy(stepsDone = done).arrived() else next.copy(stepsDone = done)
    }

    /** An explicit user choice promotes a previously suggested track to the Source. */
    fun userAdded(trackIds: List<String>): Gravity {
        val removed = trackIds.toHashSet()
        return copy(autoTrackIds = autoTrackIds.filter { it !in removed })
    }

    /** Removing a suggestion keeps it excluded on purpose, so nothing changes. */
    @Suppress("UNUSED_PARAMETER")
    fun removed(trackId: String): Gravity = this

    /** The whole destination mix becomes the new Source. */
    fun arrived(): Gravity {
        val target = destination ?: return this
        return copy(
            source = GravitySource(kind = GravitySource.KIND_REFERENCES, references = target.take(MAX_COMPONENTS)),
            destination = null,
            stepsDone = 0,
        )
    }

    /** Replaces the destination mix (null or empty clears it) and restarts the step count. */
    fun withDestination(components: List<GravityReference>?, stepsTotal: Int? = null): Gravity = copy(
        destination = components?.distinctBy { it.key }?.take(MAX_COMPONENTS)?.ifEmpty { null },
        stepsDone = 0,
        stepsTotal = (stepsTotal ?: this.stepsTotal).coerceAtLeast(1),
    )

    /** A single-component destination. */
    fun withDestination(component: GravityReference, stepsTotal: Int? = null): Gravity =
        withDestination(listOf(component), stepsTotal)

    /**
     * Adds [component] to the mix, replacing a component for the same entity. Progress is kept.
     * A full mix ([MAX_COMPONENTS]) is left unchanged.
     */
    fun withDestinationComponentAdded(component: GravityReference): Gravity {
        val current = destination ?: return withDestination(listOf(component))
        val index = current.indexOfFirst { it.key == component.key }
        val next = when {
            index >= 0 -> current.toMutableList().also { it[index] = component }
            current.size >= MAX_COMPONENTS -> return this
            else -> current + component
        }
        return copy(destination = next)
    }

    /** Removes one component; removing the last one clears the destination. */
    fun withDestinationComponentRemoved(entityType: String, entityId: String): Gravity {
        val current = destination ?: return this
        val next = current.filterNot { it.entityType == entityType && it.entityId == entityId }
        return if (next.isEmpty()) copy(destination = null, stepsDone = 0) else copy(destination = next)
    }

    fun withDestinationComponentWeight(entityType: String, entityId: String, weight: Double): Gravity {
        val current = destination ?: return this
        val safe = if (weight.isFinite()) weight.coerceIn(MIN_COMPONENT_WEIGHT, MAX_COMPONENT_WEIGHT) else 1.0
        return copy(destination = current.map {
            if (it.entityType == entityType && it.entityId == entityId) it.copy(weight = safe) else it
        })
    }

    fun withSource(source: GravitySource): Gravity = copy(source = source)

    fun withStepsTotal(stepsTotal: Int): Gravity {
        val total = stepsTotal.coerceAtLeast(1)
        val next = copy(stepsTotal = total)
        return if (destination != null && stepsDone >= total) next.arrived() else next
    }

    fun withKnobs(knobs: GravityKnobs): Gravity = copy(knobs = knobs)

    fun withDiagnostics(diagnostics: GravityDiagnostics?): Gravity = copy(lastDiagnostics = diagnostics)

    fun buildContinuationRequest(tracksIds: List<String>, currentIndex: Int, count: Int): ContinuationRequest {
        val auto = autoTrackIds.toHashSet()
        val userChosen = tracksIds.filter { it !in auto }.distinct()
        val recent = if (tracksIds.isEmpty()) emptyList() else {
            val end = currentIndex.coerceIn(0, tracksIds.lastIndex)
            tracksIds.subList((end - (RECENT_COUNT - 1)).coerceAtLeast(0), end + 1)
        }
        val isReferenceSource = source.kind == GravitySource.KIND_REFERENCES
        val target = destination
        return ContinuationRequest(
            contextTrackIds = tracksIds.takeLast(10),
            sourceTrackIds = if (isReferenceSource) emptyList() else sampleEvenly(userChosen, SOURCE_SAMPLE_CAP),
            sourceReferences = if (isReferenceSource) source.references.map { it.toContinuationReference() } else emptyList(),
            recentTrackIds = recent,
            recencyWeight = knobs.recencyWeight,
            excludeTrackIds = (tracksIds + autoTrackIds).distinct(),
            count = count,
            destination = target?.map { it.toContinuationReference() },
            progress = if (target != null) progress() else null,
            criteria = knobs.criteria?.map { ContinuationRequest.Criterion(it.namespace, it.weight) },
            diversity = knobs.diversity,
            randomness = knobs.randomness,
            mode = knobs.mode,
            away = knobs.away.map { it.toContinuationReference() },
        )
    }

    companion object {
        const val VERSION = 2
        const val MAX_COMPONENTS = 8
        const val MIN_COMPONENT_WEIGHT = 0.05
        const val MAX_COMPONENT_WEIGHT = 4.0
        const val DEFAULT_STEPS_TOTAL = 10
        const val AUTO_IDS_CAP = 1000
        const val SOURCE_SAMPLE_CAP = 200
        const val RECENT_COUNT = 5

        private val wireJson = Json { encodeDefaults = true }
        private val readJson = Json { ignoreUnknownKeys = true }

        fun fromWireJson(json: JsonObject): Gravity = readJson.decodeFromJsonElement(serializer(), json)

        /** Evenly strided sample that keeps order and the first element; identity when short enough. */
        fun sampleEvenly(items: List<String>, max: Int): List<String> {
            if (items.size <= max || max <= 0) return items
            return (0 until max).map { i -> items[(i.toLong() * items.size / max).toInt()] }
        }
    }
}

@Serializable
data class GravitySource(
    val kind: String = KIND_QUEUE,
    val references: List<GravityReference> = emptyList(),
) {
    companion object {
        const val KIND_QUEUE = "queue"
        const val KIND_REFERENCES = "references"
    }
}

@Serializable
data class GravityReference(
    @SerialName("entity_type") val entityType: String,
    @SerialName("entity_id") val entityId: String,
    val label: String? = null,
    val weight: Double = 1.0,
) {
    fun toContinuationReference() = ContinuationReference(entityType, entityId, weight)

    /** Identity of the referenced entity, ignoring label and weight. */
    val key: String get() = "$entityType:$entityId"

    companion object {
        const val TYPE_CONCEPT = "concept"
    }
}

/**
 * The destination mix: a JSON array of references. Anything else (a v1 single object, garbage)
 * decodes as null instead of failing, so the rest of a persisted or synced gravity survives.
 */
internal object DestinationMixSerializer : KSerializer<List<GravityReference>?> {
    private val delegate = ListSerializer(GravityReference.serializer()).nullable
    override val descriptor: SerialDescriptor = delegate.descriptor

    override fun deserialize(decoder: Decoder): List<GravityReference>? {
        val json = decoder as? JsonDecoder ?: return delegate.deserialize(decoder)
        val element = json.decodeJsonElement()
        if (element !is JsonArray) return null
        return runCatching { json.json.decodeFromJsonElement(ListSerializer(GravityReference.serializer()), element) }
            .getOrNull()
            ?.ifEmpty { null }
    }

    override fun serialize(encoder: Encoder, value: List<GravityReference>?) {
        val json = encoder as? JsonEncoder ?: return delegate.serialize(encoder, value)
        json.encodeJsonElement(
            if (value == null) JsonNull
            else json.json.encodeToJsonElement(ListSerializer(GravityReference.serializer()), value)
        )
    }
}

@Serializable
data class GravityCriterion(
    val namespace: String,
    val weight: Double,
)

/** Null means "server default"; the key is then omitted from the continuation request. */
@Serializable
data class GravityKnobs(
    @SerialName("recency_weight") val recencyWeight: Double? = null,
    val criteria: List<GravityCriterion>? = null,
    val diversity: Double? = null,
    val randomness: Double? = null,
    val mode: String? = null,
    val away: List<GravityReference> = emptyList(),
)

/** The continuation response minus `track_ids`, kept for the steering UI. */
@Serializable
data class GravityDiagnostics(
    @SerialName("recency_weight") val recencyWeight: Double? = null,
    val progress: Double? = null,
    val namespaces: List<GravityNamespaceDiagnostics> = emptyList(),
    val at: Long = 0,
)

@Serializable
data class GravityNamespaceDiagnostics(
    val namespace: String,
    val weight: Double = 1.0,
    @SerialName("source_to_destination") val sourceToDestination: Double? = null,
    @SerialName("query_to_source") val queryToSource: Double? = null,
    @SerialName("query_to_destination") val queryToDestination: Double? = null,
    @SerialName("destination_components") val destinationComponents: List<GravityComponentDiagnostics> = emptyList(),
)

/** How close the last continuation query was to one destination component. */
@Serializable
data class GravityComponentDiagnostics(
    @SerialName("entity_type") val entityType: String,
    @SerialName("entity_id") val entityId: String,
    val similarity: Double? = null,
)
