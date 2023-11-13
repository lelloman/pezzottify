package com.lelloman.pezzottify.server.model

import jakarta.persistence.Entity
import jakarta.persistence.Id

@Entity
data class User(
    @Id
    val id: String,
    val password: String,
)