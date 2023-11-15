package com.lelloman.pezzottify.server.service

import com.lelloman.pezzottify.server.ImagesRepository
import com.lelloman.pezzottify.server.model.Image
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.stereotype.Service
import org.springframework.web.multipart.MultipartFile
import java.io.BufferedInputStream

@Service
class ImageUploader(
    @Autowired private val imageDecoder: ImageDecoder,
    @Autowired private val storage: FileStorageService,
    @Autowired private val imagesRepo: ImagesRepository,
) {

    fun newOperation() = object : UploadOperation {
        private val pendingImages = mutableListOf<Image>()

        override fun createImage(multipartFile: MultipartFile) =
            multipartFile.inputStream.let(::BufferedInputStream).let { imageIs ->
                val imageSpecs = imageDecoder.decode(imageIs) ?: throw DecodeException()
                imageIs.let(storage::create).let { (id, size) ->
                    val imageToSave = Image(
                        id = id,
                        size = size,
                        width = imageSpecs.width,
                        height = imageSpecs.height,
                        type = imageSpecs.type,
                    )
                    imagesRepo.save(imageToSave).also(pendingImages::add)
                }
            }

        override fun deleteImage(id: String) = imagesRepo.deleteById(id)

        override fun aborted() = imagesRepo.deleteAll(pendingImages)

        override fun succeeded() {
            imagesRepo.saveAll(pendingImages.map { it.copy(orphan = false) })
        }
    }

    interface UploadOperation {
        fun createImage(multipartFile: MultipartFile): Image
        fun deleteImage(id: String)
        fun aborted()
        fun succeeded()
    }

    class DecodeException : RuntimeException()
}