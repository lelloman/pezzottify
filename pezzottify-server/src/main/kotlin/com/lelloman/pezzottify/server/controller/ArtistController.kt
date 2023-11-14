package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.ArtistRepository
import com.lelloman.pezzottify.server.ImagesRepository
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.Image
import com.lelloman.pezzottify.server.service.FileStorageService
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.*
import org.springframework.web.multipart.MultipartFile
import java.io.BufferedInputStream
import kotlin.jvm.optionals.getOrNull

@RestController
@RequestMapping("/api")
class ArtistController(
    @Autowired private val repo: ArtistRepository,
    @Autowired private val imagesRepo: ImagesRepository,
    @Autowired private val storage: FileStorageService,
    @Autowired private val imageDecoder: ImageDecoder,
) {

    @GetMapping("/artists")
    fun all(): Iterable<Artist> {
        return repo.findAll()
    }

    @GetMapping("/artist/{id}")
    fun getArtist(@PathVariable("id") id: String): ResponseEntity<Artist> {
        return when (val model = repo.findById(id).getOrNull()) {
            null -> ResponseEntity(HttpStatus.NOT_FOUND)
            else -> ResponseEntity.accepted().body(model)
        }
    }

    @PostMapping("/artist", consumes = ["multipart/form-data"])
    fun newArtist(
        @RequestPart("artist") artist: Artist,
        @RequestParam("image") image: MultipartFile?,
    ): ResponseEntity<Artist> {
        if (artist.displayName.isBlank()) {
            return ResponseEntity(HttpStatus.BAD_REQUEST)
        }


        val createdImage = image?.inputStream?.let(::BufferedInputStream)?.let { imageIs ->
            val imageSpecs = imageDecoder.decode(imageIs) ?: return ResponseEntity(HttpStatus.BAD_REQUEST)
            imageIs.let(storage::create).let { (id, size) ->
                val imageToSave = Image(
                    id = id,
                    size = size,
                    width = imageSpecs.width,
                    height = imageSpecs.height,
                    type = imageSpecs.type,
                )
                imagesRepo.save(imageToSave)
            }
        }

        val artistToSave = artist.copy(image = createdImage)
        val response = ResponseEntity(repo.save(artistToSave), HttpStatus.CREATED)
        createdImage?.let { imagesRepo.save(it.copy(orphan = false)) }
        return response
    }

    @PutMapping("/artist", consumes = ["multipart/form-data"])
    fun replace(
        @RequestPart("artist") artist: Artist,
        @RequestParam("image") image: MultipartFile?,
    ): ResponseEntity<Artist> {
        val foundArtist = repo.findById(artist.id).getOrNull() ?: return ResponseEntity(HttpStatus.NOT_FOUND)

        if (artist.displayName.isBlank()) {
            return ResponseEntity(HttpStatus.BAD_REQUEST)
        }

        val createdImage = image?.inputStream?.let { imageIs ->
            val imageSpecs = imageDecoder.decode(imageIs) ?: return ResponseEntity(HttpStatus.BAD_REQUEST)
            imageIs.let(storage::create).let { (id, size) ->
                val imageToSave = Image(
                    id = id,
                    size = size,
                    width = imageSpecs.width,
                    height = imageSpecs.height,
                    type = imageSpecs.type,
                )
                imagesRepo.save(imageToSave)
            }
        }
        if (createdImage != null && foundArtist.image != null) {
            imagesRepo.delete(foundArtist.image)
        }

        val artistToSave = when {
            image == null -> artist
            else -> artist.copy(image = createdImage)
        }
        val response = ResponseEntity(repo.save(artistToSave), HttpStatus.ACCEPTED)
        createdImage?.let { imagesRepo.save(it.copy(orphan = false)) }
        return response
    }
}