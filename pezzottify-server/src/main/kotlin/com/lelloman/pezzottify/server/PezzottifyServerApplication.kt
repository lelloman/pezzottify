package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.AudioTrack
import org.slf4j.LoggerFactory
import org.springframework.boot.CommandLineRunner
import org.springframework.boot.autoconfigure.EnableAutoConfiguration
import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.security.core.userdetails.User
import org.springframework.security.core.userdetails.UserDetails
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.provisioning.InMemoryUserDetailsManager
import javax.sql.DataSource

@Configuration
class DevConfig {

    private val log = LoggerFactory.getLogger(this::class.java)

    @Bean
    fun demo(
        artistRepo: ArtistRepository,
        trackRepo: AudioTrackRepository,
        dataSource: DataSource
    ): CommandLineRunner = CommandLineRunner {
        val dbUrl = dataSource.connection?.metaData?.url
        log.info("")
        log.info("-------------- DEMO CLI RUNNER --------------")

        log.info("DB: $dbUrl")

        val prince = Artist(
            firstName = "",
            lastName = "",
            displayName = "Prince"
        )
        val createdPrince = artistRepo.save(prince)
        log.info("Created prince: $createdPrince")

        val lello = Artist(
            firstName = "Lello",
            lastName = "Vitello",
            displayName = "Lelloman",
        )
        val createdLello = artistRepo.save(lello)
        log.info("Created lello: $createdLello")

        val track1 = AudioTrack(
            size = 1234,
            durationMs = 60_000,
            artists = listOf(lello, prince),
        )
        val createdTrack1 = trackRepo.save(track1)
        log.info("Created track1: $createdTrack1")

        log.info("---------------------------------------------")
        log.info("")
    }

}


@SpringBootApplication
@EnableAutoConfiguration
class PezzottifyServerApplication {

    @Bean
    fun userDetailsService(): UserDetailsService {
        val user: UserDetails = User.withDefaultPasswordEncoder()
            .username("admin")
            .password("admin")
            .roles("ADMIN")
            .build()
        return InMemoryUserDetailsManager(user)
    }
}

fun main(args: Array<String>) {
    runApplication<PezzottifyServerApplication>(*args)
}
