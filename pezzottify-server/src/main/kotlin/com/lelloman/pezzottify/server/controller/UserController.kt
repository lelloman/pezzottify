package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.AlbumRepository
import com.lelloman.pezzottify.server.UsersRepository
import com.lelloman.pezzottify.server.controller.model.UpdateBookmarkedAlbumsRequest
import com.lelloman.pezzottify.server.controller.model.UserStateResponse
import com.lelloman.pezzottify.server.model.Album
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.security.access.annotation.Secured
import org.springframework.web.bind.annotation.*
import kotlin.jvm.optionals.getOrNull

@RestController
@RequestMapping("/api/user")
class UserController(
    @Autowired private val usersRepository: UsersRepository,
    @Autowired private val getAuthenticatedUser: GetAuthenticatedUser,
    @Autowired private val albumsRepository: AlbumRepository,
) {

    @GetMapping("/state")
    @Secured("USER")
    fun userState(): ResponseEntity<UserStateResponse> {
        val user = getAuthenticatedUser() ?: return ResponseEntity(HttpStatus.BAD_REQUEST)
        return ResponseEntity.ok(UserStateResponse(bookmarkedAlbums = user.bookmarkedAlbums, playlists = emptyList()))
    }

    @PutMapping("/albums", consumes = ["application/json"])
    @Secured("USER")
    fun addAlbums(@RequestBody request: UpdateBookmarkedAlbumsRequest): ResponseEntity<String> {
        val user = getAuthenticatedUser() ?: return ResponseEntity(HttpStatus.UNAUTHORIZED)
        val newAlbums = HashSet(user.bookmarkedAlbums)
        request.albumsIdsToAdd.forEach { albumId ->
            albumsRepository.findById(albumId).getOrNull()?.let(Album::id)?.let(newAlbums::add)
        }
        request.albumsIdsToRemove.forEach { albumId ->
            newAlbums.remove(albumId)
        }
        usersRepository.save(user.copy(bookmarkedAlbums = newAlbums))
        return ResponseEntity.ok().build()
    }
}