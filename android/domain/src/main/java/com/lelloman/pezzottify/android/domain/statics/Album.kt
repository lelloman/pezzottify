package com.lelloman.pezzottify.android.domain.statics

interface Album : StaticItem {
    val id: String
    val name: String
    val date: Int
    val displayImageId: String?
    val artistsIds: List<String>
    val discs: List<Disc>
    val availability: AlbumAvailability
    val enrichmentStatus: EntityEnrichmentStatus?
        get() = null
    val enrichment: AlbumEnrichment?
        get() = null
}

interface Disc {
    val tracksIds: List<String>
}