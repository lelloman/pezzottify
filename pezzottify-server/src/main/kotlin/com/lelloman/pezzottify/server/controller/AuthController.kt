package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.controller.model.AuthenticationRequest
import com.lelloman.pezzottify.server.controller.model.MeResponse
import com.lelloman.pezzottify.server.model.User
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.security.authentication.AuthenticationManager
import org.springframework.security.authentication.BadCredentialsException
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken
import org.springframework.security.core.context.SecurityContextHolder
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.PostMapping
import org.springframework.web.bind.annotation.RequestBody
import org.springframework.web.bind.annotation.RestController

@RestController
class AuthController(
    @Autowired private val authenticationManager: AuthenticationManager,
    @Autowired private val usersRepository: UserDetailsService,
) {

    private val authenticatedUser
        get() = SecurityContextHolder
            .getContext()
            .authentication?.name?.let { usersRepository.loadUserByUsername(it) as? User }


    /*@PostMapping("/authenticate")
    fun authenticate(@RequestBody body: AuthenticationRequest): ResponseEntity<String> {
        try {
            val authentication =
                authenticationManager.authenticate(UsernamePasswordAuthenticationToken(body.username, body.password));
            val principalUser = authentication.principal as? User ?: throw BadCredentialsException("")
            val token = jwtUtil.createToken(principalUser)
            return ResponseEntity.ok(token);

        } catch (e: BadCredentialsException) {
            badRequest("Invalid username or password")
        } catch (e: Throwable) {

        }
        return ResponseEntity(HttpStatus.BAD_REQUEST)
    }*/

    @GetMapping("/me")
    fun me(): ResponseEntity<MeResponse> {
        val user = authenticatedUser
        val response = when {
            user == null -> MeResponse.Nobody
            user.roles.containsAll(listOf(User.Role.ADMIN, User.Role.USER)) -> MeResponse.AdminAndUser(user.username)
            user.roles.contains(User.Role.ADMIN) -> MeResponse.Admin(user.username)
            user.roles.contains(User.Role.USER) -> MeResponse.User(user.username)
            else -> MeResponse.NoRolesUser(user.username)
        }
        return ResponseEntity.ok(response)
    }
}