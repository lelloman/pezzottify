package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.Album
import com.lelloman.pezzottify.server.model.Artist
import com.lelloman.pezzottify.server.model.AudioTrack
import org.springframework.data.jpa.repository.JpaRepository

interface ArtistRepository : JpaRepository<Artist, String>

interface AudioTrackRepository : JpaRepository<AudioTrack, String>

interface AlbumRepository : JpaRepository<Album, String>