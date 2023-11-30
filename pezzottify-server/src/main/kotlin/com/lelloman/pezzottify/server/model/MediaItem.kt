package com.lelloman.pezzottify.server.model

import com.lelloman.pezzottify.server.service.FileStorageService
import jakarta.persistence.*
import org.hibernate.annotations.CreationTimestamp
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.stereotype.Component
import java.time.Instant
import java.util.*

interface MediaItem {
    val id: String
    val size: Long
    val created: Date
    val orphan: Boolean
}

@EntityListeners(MediaItemListener::class)
@Entity
data class Image(
    @Id
    override val id: String,
    override val size: Long,
    @CreationTimestamp
    override val created: Date = Date.from(Instant.now()),
    override val orphan: Boolean = true,
    val width: Int,
    val height: Int,
    val type: Type,
) : MediaItem {
    enum class Type {
        PNG, JPG
    }
}

enum class ArtistRole {
    Performer,
    Composer,
}

@Entity
data class ArtistRelation(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    val id: String = "",

    val artistId: String,

    val role: ArtistRole = ArtistRole.Performer,
)

@EntityListeners(MediaItemListener::class)
@Entity
data class AudioTrack(
    @Id
    override val id: String = "",

    override val size: Long,

    override val created: Date = Date.from(Instant.now()),

    override val orphan: Boolean = true,

    val name: String,

    val durationMs: Long,

    val sampleRate: Int,

    val bitRate: Long,

    val type: Type,

    @ManyToMany(cascade = [CascadeType.ALL])
    val artists: List<ArtistRelation> = emptyList(),
) : MediaItem {
    enum class Type {
        MP3, FLAC;

        fun mimeType() = when (this) {
            MP3 -> "audio/mpeg"
            FLAC -> "audio/x-flac"
        }
    }
}

@Component
class MediaItemListener(@Autowired private val fileStorageService: FileStorageService) {

    @PostRemove
    fun postRemove(mediaItem: MediaItem) {
        fileStorageService.remove(mediaItem.id)
    }
}