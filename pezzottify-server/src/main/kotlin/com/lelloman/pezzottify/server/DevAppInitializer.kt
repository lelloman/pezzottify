package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.AudioTrack
import com.lelloman.pezzottify.server.model.Image
import com.lelloman.pezzottify.server.model.IndividualArtist
import com.lelloman.pezzottify.server.service.FileStorageService
import org.slf4j.LoggerFactory
import org.springframework.boot.CommandLineRunner
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.context.annotation.Profile
import java.awt.Color
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.IOException
import javax.imageio.ImageIO
import javax.sql.DataSource


@Configuration
@Profile("dev")
class DevAppInitializer {

    private val log = LoggerFactory.getLogger(this::class.java)

    @Bean
    fun demo(
        artistRepo: ArtistRepository,
        trackRepo: AudioTrackRepository,
        imagesRepository: ImagesRepository,
        fileStorageService: FileStorageService,
        dataSource: DataSource
    ): CommandLineRunner = CommandLineRunner {
        val dbUrl = dataSource.connection?.metaData?.url
        log.info("")
        log.info("-------------- DEMO CLI RUNNER --------------")

        log.info("DB: $dbUrl")

        val bufferedImage = BufferedImage(128, 128, BufferedImage.TYPE_INT_RGB)
        val g = bufferedImage.graphics
        g.color = Color.BLUE
        g.fillRect(0, 0, 128, 128)
        g.color = Color.RED
        g.font = g.font.deriveFont(60f)
        g.drawString("P", 45, 82)
        val imageOutput = ByteArrayOutputStream()
        ImageIO.write(bufferedImage, "png", imageOutput)
        val imageBytes = imageOutput.toByteArray()
        val imageId = fileStorageService.create(imageBytes.inputStream())
        val image = Image(
            id = imageId.id,
            size = imageBytes.size.toLong(),
            orphan = false,
            width = 128,
            height = 128,
            type = Image.Type.PNG,
        )
        val createdImage = imagesRepository.save(image)
        log.info("Prince image: ${createdImage.id}")
        val prince = IndividualArtist(
            firstName = "", lastName = "", displayName = "Prince", image = createdImage
        )
        val createdPrince = artistRepo.save(prince)
        log.info("Created prince: $createdPrince")

        val lello = IndividualArtist(
            firstName = "Lello",
            lastName = "Vitello",
            displayName = "Lelloman",
            image = null,
        )
        val createdLello = artistRepo.save(lello)
        log.info("Created lello: $createdLello")

        val track1 = AudioTrack(
            name = "First track",
            size = 1234,
            durationMs = 60_000,
            type = AudioTrack.Type.MP3,
            bitRate = 100,
            sampleRate = 44100,
        )
        val createdTrack1 = trackRepo.save(track1)
        log.info("Created track1: $createdTrack1")

        val dummyCatalogDir = findDummyCatalogDir()
        log.info("Dummy catalog dir: ${dummyCatalogDir?.absolutePath}")
        if (dummyCatalogDir != null) {
            val creator = File(dummyCatalogDir, "create.sh")
            val processBuilder = ProcessBuilder(creator.absolutePath)
            processBuilder.directory(dummyCatalogDir)
            try {
                processBuilder.start()
            } catch (ex: IOException) {
                ex.printStackTrace()
            }
        }
        log.info("---------------------------------------------")
        log.info("")
    }

    private fun findDummyCatalogDir(file: File? = File(System.getProperty("user.dir"))): File? {
        log.info("Visiting ${file?.absolutePath}")
        return when {
            file == null -> null
            file.isDirectory && file.list()?.contains("dummy-catalog") == true -> File(file, "dummy-catalog")
            file.name == "dummy-catalog" && file.isDirectory -> file
            file.parentFile != file -> findDummyCatalogDir(file.parentFile)
            else -> null
        }
    }
}