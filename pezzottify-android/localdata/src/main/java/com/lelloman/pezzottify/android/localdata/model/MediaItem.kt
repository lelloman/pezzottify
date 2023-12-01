package com.lelloman.pezzottify.android.localdata.model

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = Image.TABLE_NAME)
data class Image(
    @PrimaryKey
    val id: String,
    val size: Long,
    val width: Int,
    val height: Int,
    val type: Type,
) {
    enum class Type {
        PNG, JPG
    }

    companion object {
        const val TABLE_NAME = "image"
    }
}

enum class ArtistRole {
    Performer,
    Composer,
}

data class ArtistRelation(
    val id: String = "",

    val artistId: String,

    val role: ArtistRole = ArtistRole.Performer,
)

@Entity(tableName = AudioTrack.TABLE_NAME)
data class AudioTrack(
    @PrimaryKey
    val id: String = "",
    val size: Long = 0L,
    val name: String = "",
    val durationMs: Long = 0L,
    val sampleRate: Int = 0,
    val bitRate: Long = 0L,
    val type: Type = Type.MP3,
    val artists: List<ArtistRelation> = emptyList(),
) {
    enum class Type {
        MP3, FLAC;
    }

    companion object {
        const val TABLE_NAME = "audio_track"
    }
}