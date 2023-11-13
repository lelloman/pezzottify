package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.ArtistRepository
import com.lelloman.pezzottify.server.model.Artist
import org.springframework.web.bind.annotation.*

@RestController
@RequestMapping("/api")
class ArtistController(
    private val repo: ArtistRepository
) {

    @GetMapping("/artists")
    fun all(): Iterable<Artist> {
        return repo.findAll()
    }

    @PostMapping("/artist")
    fun newArtist(@RequestBody artist: Artist) : Artist {
        return repo.save(artist)
    }
}