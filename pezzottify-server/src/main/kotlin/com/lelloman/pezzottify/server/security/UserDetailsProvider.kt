package com.lelloman.pezzottify.server.security

import com.lelloman.pezzottify.server.UsersRepository
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.security.core.userdetails.User
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.core.userdetails.UsernameNotFoundException
import kotlin.jvm.optionals.getOrNull

@Configuration
class InMemoryUserDetailsProvider(@Autowired private val usersRepository: UsersRepository) {

    @Bean
    fun userDetailsService() = UserDetailsService {
        when (it) {
            "admin" -> User.withDefaultPasswordEncoder().username("admin").password("admin").roles("ADMIN").build()
            "user" -> User.withDefaultPasswordEncoder().username("user").password("user").roles("USER").build()
            else -> usersRepository.getByName(it).getOrNull()
                ?: throw UsernameNotFoundException("Could not find user $it")
        }
    }
}
