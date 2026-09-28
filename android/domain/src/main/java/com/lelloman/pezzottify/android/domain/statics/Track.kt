package com.lelloman.pezzottify.android.domain.statics

interface Track : StaticItem {
    val workResolution: WorkResolution? get() = null
    val workEnrichmentStatus: EntityEnrichmentStatus? get() = null
    val id: String
    val name: String
    val albumId: String
    val artistsIds: List<String>
    val durationSeconds: Int
    val availability: TrackAvailability
    val enrichmentStatus: EntityEnrichmentStatus?
        get() = null
    val enrichment: TrackEnrichment?
        get() = null
}