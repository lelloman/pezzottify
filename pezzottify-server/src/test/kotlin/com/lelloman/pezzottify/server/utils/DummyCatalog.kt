package com.lelloman.pezzottify.server.utils

import com.lelloman.pezzottify.server.controller.model.CreateAlbumRequest
import com.lelloman.pezzottify.server.controller.model.CreateBandRequest
import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.model.BandArtist
import com.lelloman.pezzottify.server.model.IndividualArtist
import com.lelloman.pezzottify.server.utils.AudioSample.FLAC
import com.lelloman.pezzottify.server.utils.AudioSample.MP3

data class DummyCatalog(
    val individualArtists: List<IndividualArtist>,
    val bands: List<BandArtist>,
    val albums: List<Album>,
) {
    companion object {
        fun create(httpClient: HttpClient): DummyCatalog {
            httpClient.performAdminLogin()
            val individuals = IntArray(10) { it }.map { index ->
                val artist = IndividualArtist(
                    displayName = "Artist $index", firstName = "First $index", lastName = "Last $index"
                )
                httpClient.createArtist(artist).addFile("image", mockPng(100 + index, 100 + index)).execute()
                    .assertStatus2xx().parsedBody<IndividualArtist>()
            }

            val band1 =
                httpClient.createArtist(CreateBandRequest("Band 1", listOf(individuals[0].id, individuals[2].id)))
                    .addFile("image", mockPng()).execute().parsedBody<BandArtist>()

            val band2 = httpClient.createArtist(
                CreateBandRequest(
                    "Band 2", listOf(individuals[3].id, individuals[4].id, individuals[5].id)
                )
            ).addFile("image", mockPng()).execute().parsedBody<BandArtist>()

            val album1Request = CreateAlbumRequest(
                "Album 1",
                artistsIds = listOf(individuals[0].id, band1.id),
                audioTracksDefs = listOf("Track 1", "Track 2").map { CreateAlbumRequest.AudioTrackDef(name = it) }
            )
            val album1: Album = httpClient.multipartPost("/api/album").addJsonField("album", album1Request)
                .addFiles("audioTracks", album1Request.audioTracksDefs.map { it.name }, listOf(MP3, FLAC))
                .addFile("cover", mockPng()).execute().assertStatus2xx().parsedBody()

            val album2Request = CreateAlbumRequest(
                "Album 2",
                artistsIds = listOf(individuals[1].id),
                audioTracksDefs = listOf("Track 1", "Track 2").map { CreateAlbumRequest.AudioTrackDef(name = it) }
            )
            val album2: Album = httpClient.multipartPost("/api/album").addJsonField("album", album2Request)
                .addFiles("audioTracks", album2Request.audioTracksDefs.map { it.name }, listOf(MP3, MP3))
                .addFile("cover", mockPng()).execute().assertStatus2xx().parsedBody()

            return DummyCatalog(
                individualArtists = individuals,
                bands = listOf(band1, band2),
                albums = listOf(album1, album2),
            )
        }
    }
}