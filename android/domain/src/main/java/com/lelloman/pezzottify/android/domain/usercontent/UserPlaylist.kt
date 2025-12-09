package com.lelloman.pezzottify.android.domain.usercontent

interface UserPlaylist {
    val id: String
    val name: String
    val trackIds: List<String>
}
