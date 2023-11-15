package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.ArtistRepository
import com.lelloman.pezzottify.server.controller.model.CreateBandRequest
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.BandArtist
import com.lelloman.pezzottify.server.model.IndividualArtist
import com.lelloman.pezzottify.server.service.ImageUploader
import org.jetbrains.annotations.TestOnly
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
        @RequestPart("individual") individual: IndividualArtist?,
        @RequestPart("band") band: CreateBandRequest?,
        @RequestParam("image") image: MultipartFile?,
    ): ResponseEntity<Artist> {
        val imagesUpload = imageUploader.newOperation()
        if ((individual == null && band == null) || (individual != null && band != null)) {
            badRequest("Must provide either an individual or a band.")
        }
        try {
            val createdImage = image?.let(imagesUpload::createImage)
            val saved = if (individual != null) {
                if (individual.displayName.isBlank()) {
                    badRequest("Artist's display name cannot be blank.")
                }

                repo.save(individual.copy(image = createdImage))
            } else {
                if (band!!.membersIds.isEmpty()) badRequest("Must provide at least one member.")
                val members = band.membersIds.map {
                    val found = repo.findById(it).getOrNull()
                    if (found == null) {
                        notFound("Could not find member with id $it")
                    }

                    found!!
                }
                val bandToSave = BandArtist(
                    displayName = band.displayName,
                    members = members,
                    image = createdImage,
                )
                repo.save(bandToSave)
            }
            val response = ResponseEntity(saved, HttpStatus.CREATED)
            imagesUpload.succeeded()
            return response
        } catch (e: Throwable) {
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