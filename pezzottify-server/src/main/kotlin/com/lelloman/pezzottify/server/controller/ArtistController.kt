package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.ArtistRepository
import com.lelloman.pezzottify.server.controller.model.CreateBandRequest
import com.lelloman.pezzottify.server.controller.model.UpdateBandRequest
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.BandArtist
import com.lelloman.pezzottify.server.model.IndividualArtist
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
    @Secured("ADMIN")
    fun newArtist(
        @RequestPart("individual") individual: IndividualArtist?,
        @RequestPart("band") band: CreateBandRequest?,
        @RequestParam("image") image: MultipartFile?,
    ): ResponseEntity<Artist> {
        if ((individual == null).xor(band == null).not()) {
            badRequest("Must provide either an individual or a band.")
        }

        val imagesUpload = imageUploader.newOperation()
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
    @Secured("ADMIN")
    fun replace(
        @RequestPart("individual") individual: IndividualArtist?,
        @RequestPart("band") band: UpdateBandRequest?,
        @RequestParam("image") image: MultipartFile?,
    ): ResponseEntity<Artist> {
        if ((individual == null).xor(band == null).not()) {
            badRequest("Must provide either an individual or a band.")
        }

        val imagesUpload = imageUploader.newOperation()
        val idToFind = individual?.id ?: band!!.id
        try {
            val foundArtist = repo.findById(idToFind).getOrNull() ?: return ResponseEntity(HttpStatus.NOT_FOUND)
            val foundHasRequestedType =
                (foundArtist is BandArtist && band != null) || (foundArtist is IndividualArtist && individual != null)
            if (!foundHasRequestedType) {
                badRequest("Wrong artist type with given id.")
                throw Throwable("Unreachable")
            }

            val createdImage = image?.let(imagesUpload::createImage)
            if (individual?.displayName?.isBlank() == true || band?.displayName?.isBlank() == true) {
                badRequest("Artist's display name cannot be blank.")
            }

            val nullImageIdRequested = individual?.image?.id == null && band?.imageId == null
            val foundArtistImage = when (foundArtist) {
                is IndividualArtist -> foundArtist.image
                is BandArtist -> foundArtist.image
                else -> null
            }
            if (foundArtistImage != null) {
                val replacedImage = createdImage != null
                val deletedImage = createdImage == null && nullImageIdRequested
                if (replacedImage || deletedImage) {
                    val imageId = foundArtistImage.id
                    when (foundArtist) {
                        is IndividualArtist -> repo.save(foundArtist.copy(image = null))
                        is BandArtist -> repo.save(foundArtist.copy(image = null))
                    }
                    imagesUpload.deleteImage(imageId)
                }
            }

            val saved = if (individual != null) {
                if (foundArtist !is IndividualArtist) throw IllegalStateException()

                val artistToSave = when {
                    image == null -> individual
                    else -> individual.copy(image = createdImage)
                }
                repo.save(artistToSave)
            } else {
                if (band !is UpdateBandRequest) throw IllegalStateException()
                if (band.membersIds.isEmpty()) badRequest("Must provide at least one member.")
                val members = band.membersIds.map {
                    val found = repo.findById(it).getOrNull()
                    if (found == null) {
                        notFound("Could not find member with id $it")
                    }
                    found!!
                }
                val bandToSave = BandArtist(
                    id = foundArtist.id,
                    displayName = band.displayName,
                    members = members,
                    image = if (image == null) foundArtist.image else createdImage,
                )
                repo.save(bandToSave)
            }

            val response = ResponseEntity(saved, HttpStatus.ACCEPTED)
            imagesUpload.succeeded()
            return response
        } catch (e: Throwable) {
            imagesUpload.aborted()
            return ResponseEntity(HttpStatus.BAD_REQUEST)
        }
    }
}