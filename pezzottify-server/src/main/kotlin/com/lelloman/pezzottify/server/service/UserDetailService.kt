package com.lelloman.pezzottify.server.service

import org.springframework.context.annotation.Bean
import org.springframework.security.core.userdetails.User
import org.springframework.security.core.userdetails.UserDetails
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.provisioning.InMemoryUserDetailsManager

@Bean
fun userDetailsService(): UserDetailsService {
    val user: UserDetails = User.withDefaultPasswordEncoder()
        .username("admin")
        .password("admin")
        .roles("ADMIN")
        .build()
    return InMemoryUserDetailsManager(user)
}