package com.lelloman.pezzottify.android.domain.player

/**
 * Contains track information needed for media playback and notification display.
 */
data class MediaTrackInfo(
    val id: String,
    val streamUrl: String,
    val title: String,
    val artistName: String,
    val albumName: String,
    val artworkUrl: String?,
    val durationSeconds: Int,
)
