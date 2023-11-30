package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.AlbumRepository
import com.lelloman.pezzottify.server.ArtistRepository
import com.lelloman.pezzottify.server.controller.model.CreateAlbumRequest
import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.ArtistRelation
import com.lelloman.pezzottify.server.model.ArtistRole
import com.lelloman.pezzottify.server.service.AudioTrackUploader
import com.lelloman.pezzottify.server.service.ImageUploader
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.security.access.annotation.Secured
import org.springframework.web.bind.annotation.*
import org.springframework.web.multipart.MultipartFile
import kotlin.jvm.optionals.getOrNull

@RestController
@RequestMapping("/api")
class AlbumController(
    @Autowired private val repo: AlbumRepository,
    @Autowired private val imageUploader: ImageUploader,
    @Autowired private val audioTrackUploader: AudioTrackUploader,
    @Autowired private val artistRepository: ArtistRepository,
) {

    @GetMapping("/albums")
    fun all(): Iterable<Album> {
        return repo.findAll()
    }

    @GetMapping("/album/{id}")
    fun getAlbum(@PathVariable("id") id: String): ResponseEntity<Album> {
        return when (val model = repo.findById(id).getOrNull()) {
            null -> ResponseEntity(HttpStatus.NOT_FOUND)
            else -> ResponseEntity.accepted().body(model)
        }
    }

    @DeleteMapping("/album/{id}")
    @Secured("ADMIN")
    fun deleteAlbum(@PathVariable("id") id: String): ResponseEntity<Void> {
        val foundArtist = repo.findById(id).getOrNull() ?: return ResponseEntity(HttpStatus.NOT_FOUND)
        repo.deleteById(id)
        return ResponseEntity.ok().build()
    }

    @PostMapping("/album", consumes = ["multipart/form-data"])
    @Secured("ADMIN")
    fun newAlbum(
        @RequestPart("album") albumRequest: CreateAlbumRequest,
        @RequestParam("cover") cover: MultipartFile?,
        @RequestParam("sideImages") sideImages: Array<MultipartFile>?,
        @RequestParam("audioTracks") audioTracks: Array<MultipartFile>,
    ): ResponseEntity<Album> {
        val audioTrackUpload = audioTrackUploader.newOperation()
        val imagesUpload = imageUploader.newOperation()
        if (albumRequest.audioTracksDefs.size != audioTracks.size) {
            badRequest("Sent ${audioTracks.size} audio files, but ${albumRequest.audioTracksDefs.size} track defs.")
        }
        if (albumRequest.artistsIds.isEmpty()) {
            badRequest("At least one artist id must be provided.")
        }
        val artists = mutableListOf<Artist>()
        albumRequest.artistsIds.map { artistId ->
            val artist = artistRepository.findById(artistId).getOrNull()
            if (artist == null) badRequest("Could not find artist with id $artistId")
            artists.add(artist!!)
        }

        return try {
            val createdCover = cover?.let(imagesUpload::createImage)
            val createdSideImages = sideImages?.map(imagesUpload::createImage)
            val createdAudioTracks = audioTracks.mapIndexed { index, multipartFile ->
                val trackDef = albumRequest.audioTracksDefs[index]
                val trackArtists = if (trackDef.artists.isNotEmpty()) {
                    trackDef.artists
                } else {
                    albumRequest.artistsIds.map { artistId ->
                        ArtistRelation(artistId = artistId, role = ArtistRole.Performer)
                    }
                }
                audioTrackUpload.createAudioTrack(multipartFile, trackDef.name, trackArtists)
            }
            val albumToCreate = Album(
                name = albumRequest.name,
                coverImage = createdCover,
                sideImages = createdSideImages.orEmpty(),
                artists = artists,
                audioTracks = createdAudioTracks,
            )
            val createdAlbum = repo.save(albumToCreate)
            audioTrackUpload.succeeded()
            imagesUpload.succeeded()
            ResponseEntity.ok().body(createdAlbum)
        } catch (e: Throwable) {
            audioTrackUpload.aborted()
            imagesUpload.aborted()
            throw e
        }
    }
}