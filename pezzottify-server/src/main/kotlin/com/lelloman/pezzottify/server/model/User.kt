package com.lelloman.pezzottify.server.model

import jakarta.persistence.Entity
import jakarta.persistence.Id
import jakarta.persistence.Table

@Entity
@Table(name = "UserEntity")
data class User(
    @Id
    val name: String,
    val password: String,
)