package com.lelloman.pezzottify.server.model

import jakarta.persistence.CascadeType
import jakarta.persistence.Entity
import jakarta.persistence.GeneratedValue
import jakarta.persistence.GenerationType
import jakarta.persistence.Id
import jakarta.persistence.ManyToMany
import jakarta.persistence.ManyToOne

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
    //@JoinColumn(name = "user_id")
    //@OnDelete(action = OnDeleteAction.CASCADE)
    //@JsonIgnore
    val owner: User,
) : Playlist