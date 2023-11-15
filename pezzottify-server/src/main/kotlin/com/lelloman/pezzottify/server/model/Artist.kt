package com.lelloman.pezzottify.server.model

import jakarta.persistence.*

@Entity
@Inheritance(strategy = InheritanceType.TABLE_PER_CLASS)
open class Artist(
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    open var id: String = "",

    @Column(unique = true)
    open var displayName: String = "",

    @ManyToOne(cascade = [CascadeType.ALL])
    @JoinColumn(name = "image_id", referencedColumnName = "id")
    open var image: Image? = null,
)

@Entity
class IndividualArtist(
    id: String = "",

    displayName: String,

    image: Image? = null,

    var firstName: String? = null,

    var lastName: String? = null,
) : Artist(id = id, displayName = displayName, image = image) {
    fun copy(
        displayName: String = this.displayName,
        image: Image? = this.image,
    ) = IndividualArtist(
        id = id,
        displayName = displayName,
        firstName = firstName,
        lastName = lastName,
        image = image,
    )

    override fun equals(other: Any?): Boolean {
        if (other !is IndividualArtist) {
            return super.equals(other)
        }
        return other.id == this.id && other.displayName == this.displayName && other.image == this.image && other.firstName == this.firstName && other.lastName == this.lastName
    }
}
