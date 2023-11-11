package com.lelloman.pezzottify.server.model

import jakarta.persistence.Entity
import jakarta.persistence.GeneratedValue
import jakarta.persistence.GenerationType
import jakarta.persistence.Id

@Entity
data class Artist(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    val id: String,
    val firstName: String?,
    val lastName: String?,
    val displayName: String,
)

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
    override val id: String,
    override val size: Long,
    override val created: Long,
    override val uri: String,
    val durationMs: Int,
) : MediaItem