package com.lelloman.pezzottify.server.model

import jakarta.persistence.*

interface Playlist {
    val id: String
    val audioTracks: List<AudioTrack>
    val name: String
}

@Entity
data class Album(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    override val id: String = "",

    override val name: String,

    @ManyToMany(cascade = [CascadeType.ALL])
    override val audioTracks: List<AudioTrack> = emptyList(),

    @ManyToOne(cascade = [CascadeType.ALL])
    val coverImage: Image? = null,

    @ManyToMany(cascade = [CascadeType.ALL])
    val sideImages: List<Image> = emptyList(),

    @ManyToMany
    val artists: List<Artist>,
) : Playlist

@Entity
data class UserPlayList(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    override val id: String,

    override val name: String,

    @ManyToMany
    override val audioTracks: List<AudioTrack>,

    @ManyToOne
    val owner: User,
) : Playlist