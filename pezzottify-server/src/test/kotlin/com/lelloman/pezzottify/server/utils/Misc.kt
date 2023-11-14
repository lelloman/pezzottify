package com.lelloman.pezzottify.server.utils

import com.lelloman.pezzottify.server.model.Artist
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import javax.imageio.ImageIO

class Artists : ArrayList<Artist>()

fun mockPng(): ByteArray {
    val image = BufferedImage(10, 10, BufferedImage.TYPE_INT_RGB)
    val output = ByteArrayOutputStream()
    ImageIO.write(image, "png", output)
    return output.toByteArray()
}