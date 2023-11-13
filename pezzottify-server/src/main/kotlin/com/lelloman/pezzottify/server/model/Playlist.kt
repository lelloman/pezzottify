package com.lelloman.pezzottify.server.model

import com.fasterxml.jackson.annotation.JsonIgnore
import jakarta.persistence.Entity
import jakarta.persistence.GeneratedValue
import jakarta.persistence.GenerationType
import jakarta.persistence.Id
import jakarta.persistence.JoinColumn
import jakarta.persistence.ManyToMany
import jakarta.persistence.ManyToOne
import org.hibernate.annotations.OnDelete
import org.hibernate.annotations.OnDeleteAction

interface Playlist {
    val id: String
    val audioTracks: List<AudioTrack>
    val name: String
}

@Entity
data class Album(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    override val id: String,
    override val name: String,
    @ManyToMany
    override val audioTracks: List<AudioTrack>,
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