package com.lelloman.pezzottify.android.ui.screen.steering

import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumSummary
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistSummary
import com.lelloman.pezzottify.android.domain.remoteapi.response.ResolvedSearchResult
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchSection
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackSummary

/** At most this many catalog results are offered as steering references. */
const val MAX_STEERING_SEARCH_RESULTS = 30

/**
 * Turns the streaming search sections received so far into steering references, in the
 * same relevance order as the main search screen: primary matches first, then the ranked
 * results, then the enrichment sections (popular tracks, albums, related artists).
 * Duplicates keep their highest-ranked position.
 */
fun steeringReferencesFrom(sections: List<SearchSection>): List<SteeringReference> {
    val primary = mutableListOf<SteeringReference>()
    val results = mutableListOf<SteeringReference>()
    val enrichment = mutableListOf<SteeringReference>()
    for (section in sections) {
        when (section) {
            is SearchSection.PrimaryArtist -> primary += section.item.toReference()
            is SearchSection.PrimaryAlbum -> primary += section.item.toReference()
            is SearchSection.PrimaryTrack -> primary += section.item.toReference()
            is SearchSection.Results -> results += section.items.map { it.toReference() }
            is SearchSection.MoreResults -> results += section.items.map { it.toReference() }
            is SearchSection.PopularBy -> enrichment += section.items.map { it.toReference() }
            is SearchSection.TracksFrom -> enrichment += section.items.map { it.toReference() }
            is SearchSection.AlbumsBy -> enrichment += section.items.map { it.toReference() }
            is SearchSection.RelatedArtists -> enrichment += section.items.map { it.toReference() }
            is SearchSection.Done -> Unit
        }
    }
    return (primary + results + enrichment)
        .distinctBy { it.entityType to it.entityId }
        .take(MAX_STEERING_SEARCH_RESULTS)
}

private fun artistNames(artistsIdsNames: List<List<String>>): String? =
    artistsIdsNames.mapNotNull { it.getOrNull(1) }.filter { it.isNotBlank() }
        .joinToString(", ").ifBlank { null }

private fun ResolvedSearchResult.toReference(): SteeringReference = when (this) {
    is ResolvedSearchResult.Artist -> SteeringReference("artist", id, name)
    is ResolvedSearchResult.Album -> SteeringReference("album", id, name, detail = artistNames(artistsIdsNames))
    is ResolvedSearchResult.Track -> SteeringReference("track", id, name, detail = artistNames(artistsIdsNames))
}

private fun TrackSummary.toReference() =
    SteeringReference("track", id, name, detail = artistNames.joinToString(", ").ifBlank { null })

private fun AlbumSummary.toReference() =
    SteeringReference("album", id, name, detail = artistNames.joinToString(", ").ifBlank { null })

private fun ArtistSummary.toReference() = SteeringReference("artist", id, name)
