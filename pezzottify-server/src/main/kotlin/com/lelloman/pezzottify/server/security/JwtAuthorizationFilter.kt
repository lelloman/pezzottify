package com.lelloman.pezzottify.server.security

import com.auth0.jwt.JWT
import com.auth0.jwt.algorithms.Algorithm
import jakarta.servlet.FilterChain
import jakarta.servlet.http.HttpServletRequest
import jakarta.servlet.http.HttpServletResponse
import org.springframework.security.authentication.AuthenticationManager
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken
import org.springframework.security.core.context.SecurityContextHolder
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.web.authentication.www.BasicAuthenticationFilter

const val TOKEN_PREFIX = "Bearer "
const val HEADER_STRING = "Authorization"

class JwtAuthorizationFilter(
    authenticationManager: AuthenticationManager, private val usersDetailsService: UserDetailsService
) : BasicAuthenticationFilter(authenticationManager) {

    override fun doFilterInternal(req: HttpServletRequest, res: HttpServletResponse, chain: FilterChain) {
        val header = req.getHeader(HEADER_STRING)

        if (header == null || !header.startsWith(TOKEN_PREFIX)) {
            chain.doFilter(req, res)
            return
        }

        val authentication: UsernamePasswordAuthenticationToken? = getAuthentication(req)

        SecurityContextHolder.getContext().authentication = authentication
        chain.doFilter(req, res)
    }

    private fun getAuthentication(request: HttpServletRequest): UsernamePasswordAuthenticationToken? {
        return request.getHeader(HEADER_STRING)?.let { token ->
            JWT.require(Algorithm.HMAC512(SECRET.toByteArray())).build()
                .verify(token.replace(TOKEN_PREFIX, "")).subject?.let(usersDetailsService::loadUserByUsername)
                ?.let { user ->
                    UsernamePasswordAuthenticationToken(user, null, user.authorities)
                }
        }
    }
}