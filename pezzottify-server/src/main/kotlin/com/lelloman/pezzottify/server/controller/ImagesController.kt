package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.ImagesRepository
import com.lelloman.pezzottify.server.model.Image
import com.lelloman.pezzottify.server.service.FileStorageService
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.core.io.InputStreamResource
import org.springframework.data.crossstore.ChangeSetPersister.NotFoundException
import org.springframework.http.HttpHeaders
import org.springframework.http.HttpStatus
import org.springframework.http.MediaType
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.PathVariable
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.RestController
import java.io.InputStream
import kotlin.jvm.optionals.getOrElse

@RestController
class ImagesController(
    @Autowired private val repo: ImagesRepository,
    @Autowired private val storage: FileStorageService,
) {

    @RequestMapping("/api/image/{id}")
    fun getImage(@PathVariable("id") id: String): ResponseEntity<InputStreamResource> {
        try {
            val image = repo.findById(id).getOrElse { throw NotFoundException() }
            val inputStream = storage.open(id)
            val contentType = when (image.type) {
                Image.Type.PNG -> MediaType.IMAGE_PNG
                Image.Type.JPG -> MediaType.IMAGE_JPEG
            }
            val headers = HttpHeaders().apply {
                setContentType(contentType)
                contentLength = image.size
            }
            return ResponseEntity.ok()
                .headers(headers)
                .body(InputStreamResource(inputStream))
        } catch (e: Throwable) {
            return ResponseEntity(HttpStatus.NOT_FOUND)
        }
    }
}