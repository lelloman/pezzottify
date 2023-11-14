package com.lelloman.pezzottify.server.service

import com.lelloman.pezzottify.server.model.Image
import org.springframework.stereotype.Component
import java.io.IOException
import java.io.InputStream
import javax.imageio.ImageIO
import javax.imageio.ImageReader
import javax.imageio.stream.MemoryCacheImageInputStream

@Component
class ImageDecoder {
    fun decode(inputStream: InputStream): ImageSpec? {
        return tryDecodeFormat(inputStream, "png") ?: tryDecodeFormat(inputStream, "jpg")
    }

    private fun tryDecodeFormat(inputStream: InputStream, format: String): ImageSpec? {
        ImageIO.getImageReadersByFormatName(format).forEach { reader ->
            val decoded = this.tryDecode(inputStream, reader)
            if (decoded != null) return decoded
        }
        return null
    }

    private fun tryDecode(inputStream: InputStream, reader: ImageReader): ImageSpec? {
        inputStream.mark(1_000_000)
        var spec: ImageSpec? = null
        try {
            reader.input = MemoryCacheImageInputStream(inputStream)
            val width = reader.getWidth(0)
            val height = reader.getHeight(0)
            val type = when (val formatName = reader.formatName.lowercase()) {
                "jpg", "jpeg" -> Image.Type.JPG
                "png" -> Image.Type.PNG
                else -> throw IOException("Unknown format name $formatName")
            }
            spec = ImageSpec(width = width, height = height, type = type)
        } catch (e: IOException) {
            val a = 1
        } finally {
            reader.dispose()
            inputStream.reset()
        }
        return spec
    }

    data class ImageSpec(
        val width: Int,
        val height: Int,
        val type: Image.Type,
    )
}