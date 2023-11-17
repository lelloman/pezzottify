package com.lelloman.pezzottify.server.utils

import com.lelloman.pezzottify.server.controller.model.CreateBandRequest
import com.lelloman.pezzottify.server.controller.model.UpdateBandRequest
import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.IndividualArtist
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import java.io.File
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

fun HttpClient.getArtist(id: String): HttpClient.ResponseSpec = this.get("/api/artist/$id")

fun HttpClient.createArtist(artist: IndividualArtist) = this.multipartPost("/api/artist")
    .addJsonField("individual", artist)

fun HttpClient.createArtist(artist: CreateBandRequest) = this.multipartPost("/api/artist")
    .addJsonField("band", artist)

fun HttpClient.updateArtist(artist: IndividualArtist) = this.multipartPut("/api/artist")
    .addJsonField("individual", artist)

fun HttpClient.updateArtist(artist: UpdateBandRequest) = this.multipartPut("/api/artist")
    .addJsonField("band", artist)

object AudioSample {
    val MP3 by lazy {
        File(javaClass.classLoader.getResource("small.mp3")!!.file).readBytes()
    }
    val FLAC by lazy {
        File(javaClass.classLoader.getResource("small.flac")!!.file).readBytes()
    }
}