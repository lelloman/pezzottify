package com.lelloman.pezzottify.server.model

import jakarta.persistence.Entity
import jakarta.persistence.GeneratedValue
import jakarta.persistence.GenerationType
import jakarta.persistence.Id


@Entity
data class Artist(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    val id: String = "",
    val firstName: String?,
    val lastName: String?,
    val displayName: String,
)
