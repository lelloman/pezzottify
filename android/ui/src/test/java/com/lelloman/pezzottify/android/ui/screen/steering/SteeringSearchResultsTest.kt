package com.lelloman.pezzottify.android.ui.screen.steering

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.response.AlbumSummary
import com.lelloman.pezzottify.android.domain.remoteapi.response.ArtistSummary
import com.lelloman.pezzottify.android.domain.remoteapi.response.MatchType
import com.lelloman.pezzottify.android.domain.remoteapi.response.ResolvedSearchResult
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchSection
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackSummary
import org.junit.Test

class SteeringSearchResultsTest {

    private val milesArtist = ResolvedSearchResult.Artist(id = "ar1", name = "Miles Davis")
    private val kindOfBlue = ResolvedSearchResult.Album(
        id = "al1", name = "Kind of Blue", artistsIdsNames = listOf(listOf("ar1", "Miles Davis")),
        availability = "available",
    )
    private val soWhat = ResolvedSearchResult.Track(
        id = "t1", name = "So What", duration = 562,
        artistsIdsNames = listOf(listOf("ar1", "Miles Davis"), listOf("ar2", "John Coltrane")),
        albumId = "al1", availability = "available",
    )

    @Test
    fun `primary matches come first, then results, then enrichment, without duplicates`() {
        val references = steeringReferencesFrom(
            listOf(
                // Enrichment can arrive before the remaining results.
                SearchSection.AlbumsBy(
                    targetId = "ar1",
                    items = listOf(
                        AlbumSummary(
                            id = "al2", name = "Bitches Brew", trackCount = 6,
                            artistNames = listOf("Miles Davis"), availability = "available",
                        ),
                    ),
                ),
                SearchSection.PrimaryArtist(item = milesArtist, confidence = 0.9),
                SearchSection.MoreResults(items = listOf(milesArtist, kindOfBlue, soWhat)),
                SearchSection.RelatedArtists(targetId = "ar1", items = listOf(ArtistSummary(id = "ar2", name = "John Coltrane"))),
                SearchSection.Done(totalTimeMs = 12),
            ),
        )
        assertThat(references.map { it.entityType to it.entityId }).containsExactly(
            "artist" to "ar1", "album" to "al1", "track" to "t1", "album" to "al2", "artist" to "ar2",
        ).inOrder()
    }

    @Test
    fun `albums and tracks carry their artists as detail`() {
        val references = steeringReferencesFrom(
            listOf(
                SearchSection.Results(items = listOf(milesArtist, kindOfBlue, soWhat)),
                SearchSection.PopularBy(
                    targetId = "ar1", targetType = MatchType.Artist,
                    items = listOf(
                        TrackSummary(
                            id = "t2", name = "Freddie Freeloader", durationMs = 1, albumId = "al1",
                            albumName = "Kind of Blue", artistNames = listOf("Miles Davis"),
                        ),
                    ),
                ),
            ),
        )
        assertThat(references.map { it.label to it.detail }).containsExactly(
            "Miles Davis" to null,
            "Kind of Blue" to "Miles Davis",
            "So What" to "Miles Davis, John Coltrane",
            "Freddie Freeloader" to "Miles Davis",
        ).inOrder()
    }

    @Test
    fun `results are capped`() {
        val many = (1..50).map { ResolvedSearchResult.Artist(id = "ar$it", name = "Artist $it") }
        assertThat(steeringReferencesFrom(listOf(SearchSection.Results(items = many))))
            .hasSize(MAX_STEERING_SEARCH_RESULTS)
    }
}
