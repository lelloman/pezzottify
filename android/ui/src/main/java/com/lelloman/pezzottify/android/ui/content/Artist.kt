package com.lelloman.pezzottify.android.ui.content

import com.lelloman.pezzottify.android.domain.statics.ArtistEnrichment
import com.lelloman.pezzottify.android.domain.statics.EntityEnrichmentStatus

data class Artist(
    val id: String,
    val name: String,
    val imageUrl: String?,
    val related: List<String>,
    val enrichmentStatus: EntityEnrichmentStatus? = null,
    val enrichment: ArtistEnrichment? = null,
)