package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.*
import org.springframework.data.jpa.repository.JpaRepository
import java.util.*

interface ArtistRepository : JpaRepository<Artist, String>

interface AudioTrackRepository : JpaRepository<AudioTrack, String>

interface AlbumRepository : JpaRepository<Album, String>

interface ImagesRepository : JpaRepository<Image, String>

interface UsersRepository : JpaRepository<User, String> {
    fun getByName(name: String): Optional<User>
}