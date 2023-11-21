package com.lelloman.pezzottify.remoteapi.model

interface Playlist {
    val id: String
    val audioTracks: List<AudioTrack>
    val name: String
}

data class Album(
    override val id: String = "",

    override val name: String,

    override val audioTracks: List<AudioTrack> = emptyList(),

    val coverImage: Image? = null,

    val sideImages: List<Image> = emptyList(),

    val artists: List<Artist>,
) : Playlist

data class UserPlayList(
    override val id: String,

    override val name: String,

    override val audioTracks: List<AudioTrack>,
) : Playlist

class Albums : ArrayList<Album>()