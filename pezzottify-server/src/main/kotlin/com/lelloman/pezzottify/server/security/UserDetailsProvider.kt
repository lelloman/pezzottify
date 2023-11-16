package com.lelloman.pezzottify.server.security

import com.lelloman.pezzottify.server.UsersRepository
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.security.core.userdetails.User
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.core.userdetails.UsernameNotFoundException
import org.springframework.security.crypto.bcrypt.BCryptPasswordEncoder
import org.springframework.security.crypto.password.PasswordEncoder
import kotlin.jvm.optionals.getOrNull


@Configuration
class InMemoryUserDetailsProvider(@Autowired private val usersRepository: UsersRepository) {

    private val passwordEncoder: PasswordEncoder = BCryptPasswordEncoder()

    private val adminUser by lazy {
        User.builder().username("admin").password(passwordEncoder.encode("admin")).roles("ADMIN").build()
    }

    private val regularUser by lazy {
        User.builder().username("user").password(passwordEncoder.encode("user")).roles("USER").build()
    }

    @Bean
    fun passwordEncoder() = passwordEncoder

    @Bean
    fun userDetailsService(passwordEncoder: PasswordEncoder) = UserDetailsService {
        when (it) {
            "admin" -> adminUser
            "user" -> regularUser
            else -> usersRepository.getByName(it).getOrNull()
                ?: throw UsernameNotFoundException("Could not find user $it")
        }
    }
}
