package com.lelloman.pezzottify.server.service

import com.lelloman.pezzottify.server.model.Image
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import javax.imageio.ImageIO

class ImageDecoderTest {

    private val tested = ImageDecoder()

    @Test
    fun `decodes png`() {
        val image1 = BufferedImage(10, 20, BufferedImage.TYPE_INT_RGB)
        val output = ByteArrayOutputStream()
        ImageIO.write(image1, "png", output)
        val imageBytes = output.toByteArray()
        val imageBytesIs = imageBytes.inputStream()

        val decoded = tested.decode(imageBytesIs)

        assertThat(decoded).isNotNull
        with(decoded!!) {
            assertThat(width).isEqualTo(10)
            assertThat(height).isEqualTo(20)
            assertThat(type).isEqualTo(Image.Type.PNG)
        }
        assertThat(imageBytesIs.readAllBytes()).isEqualTo(imageBytes)
    }

    @Test
    fun `decodes jpg`() {
        val image1 = BufferedImage(10, 20, BufferedImage.TYPE_INT_RGB)
        val output = ByteArrayOutputStream()
        ImageIO.write(image1, "jpg", output)
        val imageBytes = output.toByteArray()
        val imageBytesIs = imageBytes.inputStream()

        val decoded = tested.decode(imageBytesIs)

        assertThat(decoded).isNotNull
        with(decoded!!) {
            assertThat(width).isEqualTo(10)
            assertThat(height).isEqualTo(20)
            assertThat(type).isEqualTo(Image.Type.JPG)
        }
        assertThat(imageBytesIs.readAllBytes()).isEqualTo(imageBytes)
    }

    @Test
    fun `does not decode empty input`() {
        val decoded = tested.decode(byteArrayOf().inputStream())

        assertThat(decoded).isNull()
    }

    @Test
    fun `does not decode garbage input`() {
        val decoded = tested.decode(ByteArray(1000) { it.toByte() }.inputStream())

        assertThat(decoded).isNull()
    }
}