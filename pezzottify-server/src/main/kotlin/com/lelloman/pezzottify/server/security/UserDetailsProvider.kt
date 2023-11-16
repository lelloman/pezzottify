package com.lelloman.pezzottify.server.security

import com.lelloman.pezzottify.server.UsersRepository
import com.lelloman.pezzottify.server.model.User
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.context.annotation.Profile
import org.springframework.security.core.userdetails.UserDetails
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.core.userdetails.UsernameNotFoundException
import org.springframework.security.crypto.bcrypt.BCryptPasswordEncoder
import org.springframework.security.crypto.password.PasswordEncoder
import org.springframework.stereotype.Component
import kotlin.jvm.optionals.getOrNull

@Configuration
class PasswordEncoderProvider {
    @Bean
    fun passwordEncoder(): PasswordEncoder = BCryptPasswordEncoder()
}

@Component
interface StaticUsers {
    fun get(username: String): User?
}

@Component
@Profile("test", "dev")
class TestStaticUsers(passwordEncoder: PasswordEncoder) : StaticUsers {
    private val adminUser = User(
        name = "admin",
        pw = passwordEncoder.encode("admin"),
        roles = listOf(User.Role.ADMIN),
        bookmarkedAlbums = emptyList(),
    )
    private val regularUser = User(
        name = "user",
        pw = passwordEncoder.encode("user"),
        roles = listOf(User.Role.USER),
        bookmarkedAlbums = emptyList(),
    )

    private val users = mapOf(
        "admin" to adminUser,
        "user" to regularUser,
    )

    override fun get(username: String) = users[username]
}

@Component
class CompoundUserDetailsService(
    private val usersRepository: UsersRepository,
    private val staticUsers: StaticUsers,
) : UserDetailsService {
    override fun loadUserByUsername(it: String): UserDetails {
        staticUsers.get(it)?.let { return it }
        return usersRepository.getByName(it).getOrNull() ?: throw UsernameNotFoundException("Could not find user $it")
    }
}