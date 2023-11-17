package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.controller.model.MeResponse
import com.lelloman.pezzottify.server.model.User
import com.lelloman.pezzottify.server.service.ProfileService
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.ResponseEntity
import org.springframework.security.core.context.SecurityContextHolder
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.stereotype.Controller
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.ResponseBody

@Controller
class HomeController(
    @Autowired private val profileService: ProfileService,
    @Autowired private val getAuthenticatedUser: GetAuthenticatedUser,
) {

    @ResponseBody
    @RequestMapping("/")
    fun home(): String {
        return "HOME ${profileService.name}"
    }

    @GetMapping("/me")
    fun me(): ResponseEntity<MeResponse> {
        val user = getAuthenticatedUser()
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