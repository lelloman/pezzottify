package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.AlbumRepository
import com.lelloman.pezzottify.server.AudioTrackRepository
import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.model.AudioTrack
import com.lelloman.pezzottify.server.service.AudioTrackDecoder
import com.lelloman.pezzottify.server.service.FileStorageService
import org.apache.coyote.Response
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.*
import org.springframework.web.multipart.MultipartFile
import java.io.File
import java.io.FileInputStream
import java.io.IOException
import kotlin.jvm.optionals.getOrNull

@RestController
@RequestMapping("/api")
class AlbumController(
    @Autowired private val repo: AlbumRepository,
    @Autowired private val imageUploader: ImageUploader,
    @Autowired private val storageService: FileStorageService,
    @Autowired private val audioTrackRepo: AudioTrackRepository,
    @Autowired private val audioTrackDecoder: AudioTrackDecoder,
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
    fun deleteAlbum(@PathVariable("id") id: String): ResponseEntity<Void> {
        val foundArtist = repo.findById(id).getOrNull() ?: return ResponseEntity(HttpStatus.NOT_FOUND)
        repo.deleteById(id)
        return ResponseEntity.ok().build()
    }

    @PostMapping("/album", consumes = ["multipart/form-data"])
    fun newAlbum(
        @RequestPart("album") album: Album,
        @RequestParam("cover") cover: MultipartFile?,
        @RequestParam("sideImages") sideImages: Array<MultipartFile>?,
        @RequestPart("audioTracksNames") audioTracksNames: Array<String>,
        @RequestParam("audioTracks") audioTracks: Array<MultipartFile>,
    ): ResponseEntity<Album> {
        val pendingAudioTracks = mutableListOf<AudioTrack>()
        val imagesUpload = imageUploader.newOperation()
        var tmpFile: File? = null
        if (audioTracksNames.size != audioTracks.size) {
            return ResponseEntity(HttpStatus.BAD_REQUEST)
        }

        return try {
            val createdCover = cover?.let(imagesUpload::createImage)
            val createdSideImages = sideImages?.map(imagesUpload::createImage)
            audioTracksNames.forEachIndexed { i, trackName ->
                val audioTrackFile = audioTracks[i]
                tmpFile = File.createTempFile("upload", ".mp3")
                audioTrackFile.inputStream.copyTo(tmpFile!!.outputStream())
                val decoded = audioTrackDecoder.decode(tmpFile!!) ?: throw IOException()
                val creation = storageService.create(FileInputStream(tmpFile!!))

                val audioTrack = AudioTrack(
                    id = creation.id,
                    size = creation.size,
                    name = trackName,
                    durationMs = decoded.durationMs,
                    bitRate = decoded.bitRate,
                    sampleRate = decoded.sampleRate,
                    type = decoded.type,
                )
                val createdTrack = audioTrackRepo.save(audioTrack)
                pendingAudioTracks.add(createdTrack)
                tmpFile!!.delete()
                tmpFile = null
            }
            val createdAlbum = repo.save(
                album.copy(
                    audioTracks = pendingAudioTracks,
                    coverImage = createdCover,
                    sideImages = createdSideImages.orEmpty()
                )
            )
            audioTrackRepo.saveAll(pendingAudioTracks.map { it.copy(orphan = false) })
            imagesUpload.succeeded()
            ResponseEntity.ok().body(createdAlbum)
        } catch (e: Throwable) {
            audioTrackRepo.deleteAll(pendingAudioTracks)
            imagesUpload.aborted()
            tmpFile?.delete()
            ResponseEntity(HttpStatus.BAD_REQUEST)
        }
    }
}