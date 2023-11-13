package com.lelloman.pezzottify.server.model

import jakarta.persistence.*

interface MediaItem {
    val id: String
    val size: Long
    val created: Long
    val uri: String
}

@Entity
data class Image(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    override val id: String,
    override val size: Long,
    override val created: Long,
    override val uri: String,
    val width: Int,
    val height: Int,
) : MediaItem

@Entity
data class AudioTrack(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    override val id: String = "",
    override val size: Long,
    override val created: Long,
    override val uri: String,
    val durationMs: Int,
    @ManyToMany
    val artists: List<Artist>,
) : MediaItem