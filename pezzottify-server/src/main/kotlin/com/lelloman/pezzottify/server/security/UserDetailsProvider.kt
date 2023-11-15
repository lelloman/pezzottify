package com.lelloman.pezzottify.server.security

import org.springframework.beans.factory.annotation.Autowired
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.context.annotation.Profile
import org.springframework.security.core.userdetails.User
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.provisioning.InMemoryUserDetailsManager
import org.springframework.stereotype.Component

@Component
interface UserDetailsServiceFactory {
    fun create(): UserDetailsService
}

@Component
@Profile("dev", "test")
class InMemoryUserDetailsServiceFactory : UserDetailsServiceFactory {
    override fun create(): UserDetailsService = InMemoryUserDetailsManager().apply {
        createUser(User.withDefaultPasswordEncoder().username("admin").password("admin").roles("ADMIN").build())
        createUser(User.withDefaultPasswordEncoder().username("user").password("user").roles("USER").build())
    }
}

@Configuration
class InMemoryUserDetailsProvider {
    @Bean
    fun userDetailsService(@Autowired factory: UserDetailsServiceFactory) = factory.create()
}
