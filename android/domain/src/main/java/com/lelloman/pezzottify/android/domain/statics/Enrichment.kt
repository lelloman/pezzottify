package com.lelloman.pezzottify.android.domain.statics

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
data class EntityEnrichmentStatus(
    val entityType: String,
    val entityId: String,
    val status: String,
    val stage: String? = null,
    val attempts: Long = 0,
    val lastError: String? = null,
    val updatedAt: Long? = null,
    val enrichedAt: Long? = null,
    val sourceStatus: String? = null,
)

@Serializable
data class EntityTag(
    val tagType: String,
    val tag: String,
    val confidence: Double? = null,
    val source: String? = null,
)

@Serializable
data class EntityContributor(
    val contributorName: String,
    val contributorId: String? = null,
    val role: String,
    val confidence: Double? = null,
)

@Serializable
data class EntityRelation(
    val sourceEntityType: String,
    val sourceEntityId: String,
    val relationType: String,
    val targetEntityType: String? = null,
    val targetEntityId: String? = null,
    val externalTargetName: String? = null,
    val externalTargetUrl: String? = null,
    val confidence: Double? = null,
    val visible: Boolean = false,
    val evidence: JsonElement? = null,
)

@Serializable
data class ArtistEnrichmentProfile(
    val artistId: String,
    val kind: String? = null,
    val birthDate: String? = null,
    val deathDate: String? = null,
    val foundationDate: String? = null,
    val dissolutionDate: String? = null,
    val originPlace: String? = null,
    val originCountry: String? = null,
    val primaryLanguage: String? = null,
    val isPerson: Boolean? = null,
    val isGroup: Boolean? = null,
    val isComposer: Boolean? = null,
    val isPerformer: Boolean? = null,
    val isConductor: Boolean? = null,
    val isProducer: Boolean? = null,
    val confidence: Double? = null,
    val summary: String? = null,
    val bio: String? = null,
    val enrichedAt: Long = 0,
    val lastVerifiedAt: Long? = null,
    val sourceStatus: String? = null,
)

@Serializable
data class AlbumEnrichmentProfile(
    val albumId: String,
    val albumKind: String? = null,
    val originalReleaseDate: String? = null,
    val recordingStartDate: String? = null,
    val recordingEndDate: String? = null,
    val releaseCountry: String? = null,
    val label: String? = null,
    val catalogNumber: String? = null,
    val isLive: Boolean? = null,
    val isCompilation: Boolean? = null,
    val isSoundtrack: Boolean? = null,
    val isConceptAlbum: Boolean? = null,
    val isRemixAlbum: Boolean? = null,
    val isArchival: Boolean? = null,
    val confidence: Double? = null,
    val summary: String? = null,
    val notes: String? = null,
    val enrichedAt: Long = 0,
    val lastVerifiedAt: Long? = null,
    val sourceStatus: String? = null,
)

@Serializable
data class TrackEnrichmentProfile(
    val trackId: String,
    val trackKind: String? = null,
    val workTitle: String? = null,
    val compositionDate: String? = null,
    val recordingDate: String? = null,
    val language: String? = null,
    val isInstrumental: Boolean? = null,
    val isLive: Boolean? = null,
    val isCover: Boolean? = null,
    val isRemix: Boolean? = null,
    val isRemaster: Boolean? = null,
    val isArrangement: Boolean? = null,
    val movementNumber: Long? = null,
    val movementTitle: String? = null,
    val keySignature: String? = null,
    val opusNumber: String? = null,
    val catalogNumber: String? = null,
    val form: String? = null,
    val confidence: Double? = null,
    val summary: String? = null,
    val notes: String? = null,
    val performanceContext: String? = null,
    val enrichedAt: Long = 0,
    val lastVerifiedAt: Long? = null,
    val sourceStatus: String? = null,
)

@Serializable
data class ArtistEnrichment(
    val profile: ArtistEnrichmentProfile,
    val tags: List<EntityTag> = emptyList(),
    val contributors: List<EntityContributor> = emptyList(),
    val relations: List<EntityRelation> = emptyList(),
)

@Serializable
data class AlbumEnrichment(
    val profile: AlbumEnrichmentProfile,
    val tags: List<EntityTag> = emptyList(),
    val contributors: List<EntityContributor> = emptyList(),
    val relations: List<EntityRelation> = emptyList(),
)

@Serializable
data class TrackEnrichment(
    val profile: TrackEnrichmentProfile,
    val tags: List<EntityTag> = emptyList(),
    val contributors: List<EntityContributor> = emptyList(),
    val relations: List<EntityRelation> = emptyList(),
)
