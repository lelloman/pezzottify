package com.lelloman.pezzottify.server.model

import jakarta.persistence.*

@Entity
data class Artist(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    val id: String = "",

    @Column(unique = true)
    val displayName: String,

    val firstName: String? = null,

    val lastName: String? = null,

    @ManyToOne(cascade = [CascadeType.ALL])
    @JoinColumn(name = "image_id", referencedColumnName = "id")
    val image: Image? = null,
)
