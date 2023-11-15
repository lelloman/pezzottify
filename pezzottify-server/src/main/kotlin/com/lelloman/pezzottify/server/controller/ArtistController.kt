package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.ArtistRepository
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.IndividualArtist
import com.lelloman.pezzottify.server.service.ImageUploader
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.*
import org.springframework.web.multipart.MultipartFile
import kotlin.jvm.optionals.getOrNull

@RestController
@RequestMapping("/api")
class ArtistController(
    @Autowired private val repo: ArtistRepository,
    @Autowired private val imageUploader: ImageUploader,
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
        @RequestPart("individual") artist: IndividualArtist,
        @RequestParam("image") image: MultipartFile?,
    ): ResponseEntity<Artist> {
        val imagesUpload = imageUploader.newOperation()
        try {
            if (artist.displayName.isBlank()) {
                badRequest("Artist's display name cannot be blank.")
            }
            val createdImage = image?.let(imagesUpload::createImage)

            val artistToSave = artist.copy(image = createdImage)
            val response = ResponseEntity(repo.save(artistToSave) as Artist, HttpStatus.CREATED)
            imagesUpload.succeeded()
            return response
        } catch (e: ImageUploader.DecodeException) {
            imagesUpload.aborted()
            throw e
        }
    }

    @PutMapping("/artist", consumes = ["multipart/form-data"])
    fun replace(
        @RequestPart("individual") individual: IndividualArtist,
        @RequestParam("image") image: MultipartFile?,
    ): ResponseEntity<Artist> {
        val imagesUpload = imageUploader.newOperation()
        try {
            val foundArtist = repo.findById(individual.id).getOrNull() ?: return ResponseEntity(HttpStatus.NOT_FOUND)
            if (/*individual != null && */foundArtist !is IndividualArtist) {
                badRequest("Wrong artist type with given id.")
                throw Throwable("Unreachable")
            }

            if (individual.displayName.isBlank()) {
                badRequest("Artist's display name cannot be blank.")
            }

            val createdImage = image?.let(imagesUpload::createImage)
            if (foundArtist.image != null) {
                val replacedImage = createdImage != null
                val deletedImage = createdImage == null && individual.image == null
                if (replacedImage || deletedImage) {
                    val imageId = foundArtist.image!!.id
                    repo.save(foundArtist.copy(image = null))
                    imagesUpload.deleteImage(imageId)
                }
            }

            val artistToSave = when {
                image == null -> individual
                else -> individual.copy(image = createdImage)
            }
            val response = ResponseEntity(repo.save(artistToSave) as Artist, HttpStatus.ACCEPTED)
            imagesUpload.succeeded()
            return response
        } catch (e: Throwable) {
            imagesUpload.aborted()
            return ResponseEntity(HttpStatus.BAD_REQUEST)
        }
    }
}