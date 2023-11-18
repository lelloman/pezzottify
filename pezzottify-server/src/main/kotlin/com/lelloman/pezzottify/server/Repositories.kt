package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.*
import org.springframework.data.jpa.repository.JpaRepository
import org.springframework.transaction.annotation.Transactional
import java.util.*

@Transactional
interface ArtistRepository : JpaRepository<Artist, String>

@Transactional
interface AudioTrackRepository : JpaRepository<AudioTrack, String>

@Transactional
interface AlbumRepository : JpaRepository<Album, String>

@Transactional
interface ImagesRepository : JpaRepository<Image, String>

@Transactional
interface UsersRepository : JpaRepository<User, String> {
    fun getByName(name: String): Optional<User>
}