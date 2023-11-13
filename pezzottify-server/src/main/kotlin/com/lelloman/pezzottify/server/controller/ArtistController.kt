package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.ArtistRepository
import com.lelloman.pezzottify.server.ImagesRepository
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.Image
import com.lelloman.pezzottify.server.service.FileStorageService
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.web.bind.annotation.*
import org.springframework.web.multipart.MultipartFile

@RestController
@RequestMapping("/api")
class ArtistController(
    @Autowired private val repo: ArtistRepository,
    @Autowired private val imagesRepo: ImagesRepository,
    @Autowired private val storage: FileStorageService,
) {

    @GetMapping("/artists")
    fun all(): Iterable<Artist> {
        return repo.findAll()
    }

    @PostMapping("/artist", consumes = ["multipart/form-data"])
    fun newArtist(
        @RequestPart("artist") artist: Artist,
        @RequestParam("image") image: MultipartFile?,
    ): Artist {
        val createdImage = image?.inputStream
            ?.let(storage::create)
            ?.let { (id, size) ->
                val imageToSave = Image(
                    id = id,
                    size = size,
                    width = 0,
                    height = 0,
                )
                imagesRepo.save(imageToSave)
            }

        val artistToSave = artist.copy(image = createdImage)
        return repo.save(artistToSave).also {
            createdImage?.let {
                imagesRepo.save(it.copy(orphan = false))
            }
        }
    }
}