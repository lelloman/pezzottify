package com.lelloman.pezzottify.android.localdata.model

data class Image(
    val id: String,
    val size: Long,
    val width: Int,
    val height: Int,
    val type: Type,
) {
    enum class Type {
        PNG, JPG
    }
}

data class AudioTrack(
    val id: String = "",
    val size: Long,
    val name: String,
    val durationMs: Long,
    val sampleRate: Int,
    val bitRate: Long,
    val type: Type,
) {
    enum class Type {
        MP3, FLAC;
    }
}