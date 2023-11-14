package com.lelloman.pezzottify.server.utils

import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.model.Artist
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import javax.imageio.ImageIO

class Artists : ArrayList<Artist>()

class Albums : ArrayList<Album>()

fun mockPng(
    width: Int = 10,
    height: Int = 10,
): ByteArray {
    val image = BufferedImage(width, height, BufferedImage.TYPE_INT_RGB)
    val output = ByteArrayOutputStream()
    ImageIO.write(image, "png", output)
    return output.toByteArray()
}