package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.AlbumRepository
import com.lelloman.pezzottify.server.AudioTrackRepository
import com.lelloman.pezzottify.server.ImagesRepository
import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.service.FileStorageService
import com.lelloman.pezzottify.server.service.ImageDecoder
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.*
import org.springframework.web.multipart.MultipartFile
import kotlin.jvm.optionals.getOrNull

@RestController
@RequestMapping("/api")
class AlbumController(
    @Autowired private val repo: AlbumRepository,
    @Autowired private val imagesRepo: ImagesRepository,
    @Autowired private val storageService: FileStorageService,
    @Autowired private val imageDecoder: ImageDecoder,
    @Autowired private val audioTrackRepo: AudioTrackRepository,
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

    @PostMapping("/album", consumes = ["multipart/form-data"])
    fun newAlbum(
        @RequestPart("album") album: Album,
        @RequestParam("cover") cover: MultipartFile?,
        @RequestParam("sideImages") sideImages: Array<MultipartFile>?,
        @RequestPart("audioTracksNames") audioTracksNames: Array<String>,
        @RequestParam("audioTracks") audioTracks: Array<MultipartFile>,
    ): ResponseEntity<Album> {
        val pendingAudioTracks = mutableListOf<String>()

        if (audioTracksNames.size != audioTracks.size) {
            return ResponseEntity(HttpStatus.BAD_REQUEST)
        }

        return try {
            audioTracksNames.forEachIndexed { i, trackName ->
                val audioTrackFile = audioTracks[i]

            }
            val createdAlbum = repo.save(album)
            ResponseEntity.ok().body(createdAlbum)
        } catch (e: Throwable) {
            pendingAudioTracks.forEach { audioTrackRepo.deleteById(it) }
            ResponseEntity(HttpStatus.BAD_REQUEST)
        }
    }
}