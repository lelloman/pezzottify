package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.model.User
import org.springframework.security.core.context.SecurityContextHolder
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.stereotype.Component

@Component
class GetAuthenticatedUser(private val userDetailsService: UserDetailsService) {
    operator fun invoke() = try {
        SecurityContextHolder
            .getContext()
            .authentication?.name?.let { userDetailsService.loadUserByUsername(it) as? User }
    } catch (e: Throwable) {
        null
    }
}