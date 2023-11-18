package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.AudioTrackRepository
import com.lelloman.pezzottify.server.ImagesRepository
import com.lelloman.pezzottify.server.model.Image
import com.lelloman.pezzottify.server.service.FileStorageService
import org.slf4j.LoggerFactory
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.core.io.InputStreamResource
import org.springframework.data.crossstore.ChangeSetPersister.NotFoundException
import org.springframework.http.HttpHeaders
import org.springframework.http.HttpStatus
import org.springframework.http.MediaType
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.PathVariable
import org.springframework.web.bind.annotation.RequestHeader
import org.springframework.web.bind.annotation.RestController
import org.springframework.web.servlet.mvc.method.annotation.StreamingResponseBody
import java.io.InputStream
import kotlin.jvm.optionals.getOrElse

@RestController
class MediaItemsController(
    @Autowired private val imagesRepo: ImagesRepository,
    @Autowired private val audioTracksRepository: AudioTrackRepository,
    @Autowired private val storage: FileStorageService,
) {

    private val log = LoggerFactory.getLogger(this::class.java)

    @GetMapping("/api/image/{id}")
    fun getImage(@PathVariable("id") id: String): ResponseEntity<InputStreamResource> {
        try {
            val image = imagesRepo.findById(id).getOrElse { throw NotFoundException() }
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

    @GetMapping("/api/track/{id}")
    fun getTrack(
        @PathVariable("id") id: String,
        @RequestHeader(value = "Range", required = false) rangeHeader: String?,
    ): ResponseEntity<InputStreamResource> {
        try {
            val track = audioTracksRepository.findById(id).getOrElse { throw NotFoundException() }
            val headers = HttpHeaders().apply {
                set("Content-type", track.type.mimeType())
            }
            val status: Int
            val inputStream: InputStream
            if (rangeHeader != null) {
                val rangeSplit = rangeHeader.substring(6).split("-")
                val start = rangeSplit[0].toLong()
                val end = if (rangeSplit.size > 1 && rangeSplit[1].isNotBlank()) {
                    rangeSplit[1].toLong()
                } else {
                    track.size - 1
                }
                val toWrite = (end - start) + 1
                log.info("Track ${track.name} with size ${track.size} start-end $start-$end toWrite $toWrite")
                if (start >= track.size - 1) {
                    return ResponseEntity.status(416).build()
                }

                headers.contentLength = toWrite
                inputStream = storage.openAt(id, start)
                headers.add("Accept-Ranges", "bytes")
                headers.add("Content-Range", "bytes $start-$end/${track.size}");
                status = 206
            } else {
                headers.contentLength = track.size
                inputStream = storage.open(id)
                status = 200
            }
            return ResponseEntity.status(status)
                .headers(headers)
                .body(InputStreamResource(inputStream))

        } catch (e: CustomControllerException.NotFound) {
            return ResponseEntity(HttpStatus.NOT_FOUND)
        } catch (e: Throwable) {
            e.printStackTrace()
            return ResponseEntity(HttpStatus.NOT_FOUND)
        }
    }
}