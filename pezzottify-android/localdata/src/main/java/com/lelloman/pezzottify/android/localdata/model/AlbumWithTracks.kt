package com.lelloman.pezzottify.android.localdata.model

import androidx.room.Embedded

data class AlbumWithTracks(
    @Embedded
    val album: Album,
    val tracks: List<AudioTrack>,
)