package com.lelloman.pezzottify.server.model

import jakarta.persistence.*
import org.hibernate.annotations.CreationTimestamp
import java.time.Instant
import java.util.Date

interface MediaItem {
    val id: String
    val size: Long
    val created: Date
    val orphan: Boolean
}

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
) : MediaItem

@Entity
data class AudioTrack(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    override val id: String = "",
    override val size: Long,
    override val created: Date = Date.from(Instant.now()),
    override val orphan: Boolean = true,
    val durationMs: Int,
    @ManyToMany
    val artists: List<Artist>,
) : MediaItem