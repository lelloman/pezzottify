package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.controller.ImageDecoder
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.AudioTrack
import com.lelloman.pezzottify.server.model.Image
import com.lelloman.pezzottify.server.service.FileStorageService
import org.slf4j.LoggerFactory
import org.springframework.boot.CommandLineRunner
import org.springframework.boot.autoconfigure.EnableAutoConfiguration
import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.context.annotation.Profile
import org.springframework.security.core.userdetails.User
import org.springframework.security.core.userdetails.UserDetails
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.provisioning.InMemoryUserDetailsManager
import java.awt.Color
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import javax.imageio.ImageIO
import javax.sql.DataSource

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

    @Bean
    fun imageDecoder() = ImageDecoder()
}

fun main(args: Array<String>) {
    runApplication<PezzottifyServerApplication>(*args)
}
