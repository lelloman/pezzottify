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

    fun forEach(visitor: (User) -> Unit)
}

@Component
@Profile("test", "dev")
class TestStaticUsers(passwordEncoder: PasswordEncoder) : StaticUsers {
    private val adminUser = User(
        name = "admin",
        pw = passwordEncoder.encode("admin"),
        roles = setOf(User.Role.ADMIN),
        bookmarkedAlbums = emptySet(),
    )
    private val regularUser = User(
        name = "user",
        pw = passwordEncoder.encode("user"),
        roles = setOf(User.Role.USER),
        bookmarkedAlbums = emptySet(),
    )

    private val users = mapOf(
        "admin" to adminUser,
        "user" to regularUser,
    )

    override fun get(username: String) = users[username]

    override fun forEach(visitor: (User) -> Unit) {
        users.values.forEach(visitor)
    }
}

@Component
class CompoundUserDetailsService(
    private val usersRepository: UsersRepository,
    staticUsers: StaticUsers,
) : UserDetailsService {
    init {
        staticUsers.forEach {
            if (usersRepository.findById(it.name).isEmpty) {
                usersRepository.save(it)
            }
        }
    }

    override fun loadUserByUsername(it: String): UserDetails {
        return usersRepository.getByName(it).getOrNull() ?: throw UsernameNotFoundException("Could not find user $it")
    }
}