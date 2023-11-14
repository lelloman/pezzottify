package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.service.ImageDecoder
import org.springframework.boot.autoconfigure.EnableAutoConfiguration
import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication
import org.springframework.context.annotation.Bean
import org.springframework.security.core.userdetails.User
import org.springframework.security.core.userdetails.UserDetails
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.provisioning.InMemoryUserDetailsManager

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
