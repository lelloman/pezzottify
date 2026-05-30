package com.lelloman.pezzottify.android.domain.statics

interface Artist : StaticItem {
    val id: String
    val name: String
    val displayImageId: String?
    val related: List<String>
    val enrichmentStatus: EntityEnrichmentStatus?
        get() = null
    val enrichment: ArtistEnrichment?
        get() = null
}